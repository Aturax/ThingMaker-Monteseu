//! Background commands followed through provider lifecycle events, with an
//! output-file fallback for older Claude adapters. Native task state and stop
//! controls do not depend on platform-specific temporary paths.


use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path},
};

use serde::Serialize;
use serde_json::Value;

use crate::acp::updates::ToolPatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundTaskState {
    Running,
    /// It exited with code 0.
    Completed,
    /// It exited with another code, or timed out.
    Failed,
    /// It was killed, interrupted or stopped, or its session ended first.
    Stopped,
}

/// One command left running in the background.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundTask {
    /// The agent's id for it (Claude Code's `backgroundTaskId`).
    pub task_id: String,
    /// The tool call that started it.
    pub tool_call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// The provider offers an individual stop control for this task.
    pub can_stop: bool,
    /// Native state is authoritative even for tasks without a stop control.
    #[serde(skip)]
    pub native_lifecycle: bool,
    pub state: BackgroundTaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    /// How it ended, in the words of its output's last line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ending: Option<String>,
    pub started_at_unix_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at_unix_ms: Option<i64>,
}

/// Native lifecycle edges are independent of temporary-file layout (notably
/// Windows/Git Bash paths). A late tool result or progress edge cannot reopen
/// a terminal task; a terminal provider edge can correct file-based inference.
pub fn native_update(raw: &Value, tasks: &std::collections::BTreeMap<String, BackgroundTask>, now: i64) -> Option<BackgroundTask> {
    let id = raw.get("asyncTaskId")?.as_str()?.trim();
    if id.is_empty() { return None; }
    let mut task = tasks.get(id).cloned().unwrap_or_else(|| BackgroundTask {
        task_id: id.into(), tool_call_id: String::new(), command: None,
        description: None, output_path: None, can_stop: false, native_lifecycle: true,
        state: BackgroundTaskState::Running, exit_code: None, ending: None,
        started_at_unix_ms: now, ended_at_unix_ms: None,
    });
    task.native_lifecycle = true;
    if let Some(value) = raw.get("toolCallId").and_then(Value::as_str) { task.tool_call_id = value.into(); }
    if let Some(value) = raw.get("description").and_then(Value::as_str) { task.description = Some(value.into()); }
    if let Some(value) = raw.get("outputFilePath").and_then(Value::as_str) { task.output_path = Some(value.into()); }
    if let Some(value) = raw.get("canStop").and_then(Value::as_bool) { task.can_stop = value; }
    if raw.get("sessionUpdate").and_then(Value::as_str) == Some("async_task_state_update") {
        let state = match raw.get("state").and_then(Value::as_str)? {
            "running" => BackgroundTaskState::Running,
            "completed" => BackgroundTaskState::Completed,
            "failed" => BackgroundTaskState::Failed,
            "stopped" => BackgroundTaskState::Stopped,
            _ => return None,
        };
        if state != BackgroundTaskState::Running {
            if task.state != state || task.ended_at_unix_ms.is_none() {
                task.ended_at_unix_ms = Some(now);
            }
            task.state = state;
            task.ending = raw.get("summary").and_then(Value::as_str).map(str::to_string);
        }
    }
    Some(task)
}

const SPAWNED: &str = "Command running in background with ID: ";
const WRITTEN_TO: &str = "Output is being written to: ";
const OUTPUT_SUFFIX: &str = ".output";

/// The task a tool result says it left running, and the file its output
/// goes to once the result names it. The structured id comes with the result
/// live; a replayed one has only the text, which says the same.
///
/// The text counts only when it *is* the result — the notice Claude Code
/// returns instead of output. A command whose output merely contains such a
/// notice (a log, a test fixture, a transcript being grepped) ran in the
/// foreground, and is not a task.
pub fn spawned(patch: &ToolPatch) -> Option<(String, Option<String>)> {
    let structured = patch
        .meta
        .as_ref()
        .and_then(|meta| meta.pointer("/claudeCode/toolResponse/backgroundTaskId"))
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string);
    let notice = result_texts(patch).into_iter().find_map(|text| leading_notice(&text).map(str::to_string));
    let named = notice.as_deref().map(|notice| notice[SPAWNED.len()..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect::<String>()).filter(|id| !id.is_empty());
    let id = structured.or(named)?;
    let path = notice.as_deref().and_then(|notice| {
        let at = notice.find(WRITTEN_TO)?;
        let rest = &notice[at + WRITTEN_TO.len()..];
        let end = rest.find(OUTPUT_SUFFIX)? + OUTPUT_SUFFIX.len();
        Some(rest[..end].trim().to_string())
    });
    Some((id, path))
}

/// The background notice, when a result text starts with it — bare, or as
/// the adapter shows it, in a fenced block.
fn leading_notice(text: &str) -> Option<&str> {
    let body = text.trim_start();
    let body = match body.strip_prefix("```") {
        Some(fenced) => fenced.split_once('\n').map(|(_, inside)| inside).unwrap_or("").trim_start(),
        None => body,
    };
    body.starts_with(SPAWNED).then_some(body)
}

/// Each text of a tool result, separately: its raw output, then each of its
/// content blocks.
fn result_texts(patch: &ToolPatch) -> Vec<String> {
    fn collect(value: &Value, into: &mut Vec<String>) {
        match value {
            Value::String(text) => into.push(text.clone()),
            Value::Array(items) => items.iter().for_each(|item| collect(item, into)),
            Value::Object(object) => {
                if let Some(text) = object.get("text") {
                    collect(text, into);
                }
                if let Some(content) = object.get("content") {
                    collect(content, into);
                }
            }
            _ => {}
        }
    }
    let mut texts = Vec::new();
    if let Some(raw) = &patch.raw_output {
        collect(raw, &mut texts);
    }
    if let Some(content) = &patch.content {
        collect(content, &mut texts);
    }
    texts
}

/// How a task's output says it ended, from its last line; `None` while it runs.
pub fn ending(tail: &str) -> Option<(BackgroundTaskState, Option<i64>, String)> {
    let last = tail.lines().rev().map(str::trim).find(|line| !line.is_empty())?;
    let inner = last.strip_prefix('[')?.strip_suffix(']')?;
    if let Some(code) = inner.strip_prefix("exited with code ") {
        let code = code.trim().parse::<i64>().ok();
        let state = if code == Some(0) { BackgroundTaskState::Completed } else { BackgroundTaskState::Failed };
        return Some((state, code, last.to_string()));
    }
    if inner.starts_with("timed out") {
        return Some((BackgroundTaskState::Failed, None, last.to_string()));
    }
    if inner == "killed" || inner == "interrupted" || inner.starts_with("stopped") {
        return Some((BackgroundTaskState::Stopped, None, last.to_string()));
    }
    None
}

/// Whether `path` is a background task's output file: a `.output` file in a
/// `tasks` folder of Claude Code's own folder under the temporary directory.
/// The path comes from the agent's text, so nothing else is read through it.
pub fn is_task_output(path: &Path) -> bool {
    let Ok(real) = path.canonicalize() else { return false };
    if real.extension().and_then(|ext| ext.to_str()) != Some("output") || real.parent().and_then(|parent| parent.file_name()).and_then(|name| name.to_str()) != Some("tasks") {
        return false;
    }
    let temps = [Path::new("/tmp").canonicalize().ok(), Path::new("/private/tmp").canonicalize().ok(), std::env::temp_dir().canonicalize().ok()];
    temps.iter().flatten().any(|temp| {
        real.strip_prefix(temp).ok().and_then(|inside| inside.components().next()).is_some_and(|first| matches!(first, Component::Normal(name) if (name == "claude" || name.to_string_lossy().starts_with("claude-"))))
    })
}

/// The file's length, and at most its last `limit` bytes as text, starting
/// at a line when the cut falls inside one.
pub fn read_tail(path: &Path, limit: u64) -> std::io::Result<(u64, String)> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let start = length.saturating_sub(limit);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity((length - start) as usize);
    file.take(limit).read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0
        && let Some(newline) = text.find('\n')
    {
        text.drain(..=newline);
    }
    Ok((length, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn patch(meta: Option<Value>, raw_output: Option<Value>) -> ToolPatch {
        ToolPatch { tool_call_id: Some("toolu_1".into()), meta, raw_output, ..ToolPatch::default() }
    }

    const RESULT: &str = "Command running in background with ID: bapnhs6hh. Output is being written to: /private/tmp/claude-501/-proj/6d04cf96/tasks/bapnhs6hh.output. You will be notified when it completes.";

    #[test]
    fn a_backgrounded_result_names_its_task_and_its_output_file() {
        // Live: the structured id first, the text with the file after it.
        let structured = patch(Some(json!({"claudeCode": {"toolResponse": {"stdout": "", "backgroundTaskId": "bapnhs6hh"}}})), None);
        assert_eq!(spawned(&structured), Some(("bapnhs6hh".into(), None)));
        // Replayed: the text alone says both.
        let text = patch(None, Some(Value::String(RESULT.into())));
        assert_eq!(spawned(&text), Some(("bapnhs6hh".into(), Some("/private/tmp/claude-501/-proj/6d04cf96/tasks/bapnhs6hh.output".into()))));
        let content = ToolPatch { content: Some(json!([{"type": "content", "content": {"type": "text", "text": format!("```console\n{RESULT}\n```")}}])), ..patch(None, None) };
        assert_eq!(spawned(&content).map(|(id, _)| id), Some("bapnhs6hh".into()));
        assert_eq!(spawned(&patch(None, Some(Value::String("BG-DONE".into())))), None);
        // A foreground command whose output carries such a notice — a grep
        // through a captured stream, cut short or whole — left nothing running.
        let grepped = format!("EVT {{\"rawOutput\":\"{RESULT}\"}}");
        assert_eq!(spawned(&patch(None, Some(Value::String(grepped)))), None);
        let cut = patch(None, Some(Value::String("EVT {\"text\":\"```console\\nCommand running in background with ID: bapn".into())));
        assert_eq!(spawned(&cut), None);
        let in_content = ToolPatch { content: Some(json!([{"type": "content", "content": {"type": "text", "text": format!("```console\nlog line\n{RESULT}\n```")}}])), ..patch(None, None) };
        assert_eq!(spawned(&in_content), None);
    }

    #[test]
    fn native_lifecycle_finishes_without_reading_windows_output_paths() {
        let mut tasks = std::collections::BTreeMap::new();
        let spawn = json!({"sessionUpdate": "async_task_spawned", "asyncTaskId": "win-task", "toolCallId": "bash1", "canStop": true, "outputFilePath": r"C:\Users\test\AppData\Local\Temp\claude\tasks\win-task.output"});
        let task = native_update(&spawn, &tasks, 100).unwrap();
        assert!(task.can_stop);
        tasks.insert(task.task_id.clone(), task);
        let end = json!({"sessionUpdate": "async_task_state_update", "asyncTaskId": "win-task", "state": "completed", "summary": "done"});
        let task = native_update(&end, &tasks, 200).unwrap();
        assert_eq!(task.state, BackgroundTaskState::Completed);
        assert_eq!(task.started_at_unix_ms, 100);
        assert_eq!(task.ended_at_unix_ms, Some(200));
        tasks.insert(task.task_id.clone(), task);
        // Late spawn/progress cannot put a finished task back in running.
        assert_eq!(native_update(&spawn, &tasks, 300).unwrap().state, BackgroundTaskState::Completed);
        for state in ["failed", "stopped"] {
            let edge = json!({"sessionUpdate": "async_task_state_update", "asyncTaskId": "win-task", "state": state});
            assert_ne!(native_update(&edge, &tasks, 400).unwrap().state, BackgroundTaskState::Running);
        }
    }

    #[test]
    fn the_last_line_says_how_it_ended() {
        assert_eq!(ending("BG-DONE\n\n[exited with code 0]\n"), Some((BackgroundTaskState::Completed, Some(0), "[exited with code 0]".into())));
        assert_eq!(ending("error\n[exited with code 2]"), Some((BackgroundTaskState::Failed, Some(2), "[exited with code 2]".into())));
        assert_eq!(ending("[timed out after 10 minutes]").map(|(state, ..)| state), Some(BackgroundTaskState::Failed));
        for line in ["[killed]", "[interrupted]", "[stopped: the command printed too much]"] {
            assert_eq!(ending(line).map(|(state, ..)| state), Some(BackgroundTaskState::Stopped), "{line}");
        }
        assert_eq!(ending("still going\n[progress 40%] compiling"), None);
        assert_eq!(ending(""), None);
    }

    #[test]
    fn only_claude_codes_task_files_are_read() {
        let temp = std::env::temp_dir().join(format!("claude-test-{}", std::process::id())).join("proj/sess/tasks");
        std::fs::create_dir_all(&temp).unwrap();
        let output = temp.join("abc.output");
        std::fs::write(&output, "line 1\nline 2\n[exited with code 0]\n").unwrap();
        assert!(is_task_output(&output));
        let elsewhere = tempfile::tempdir().unwrap();
        let stranger = elsewhere.path().join("tasks").join("x.output");
        std::fs::create_dir_all(stranger.parent().unwrap()).unwrap();
        std::fs::write(&stranger, "secret").unwrap();
        assert!(!is_task_output(&stranger), "not under a claude- folder");
        assert!(!is_task_output(&temp.join("../tasks/missing.output")));
        // The cut falls inside "line 2", so the tail starts at the next line.
        let (length, tail) = read_tail(&output, 25).unwrap();
        assert_eq!(length, 35);
        assert_eq!(tail, "[exited with code 0]\n");
        assert_eq!(read_tail(&output, 1000).unwrap().1, "line 1\nline 2\n[exited with code 0]\n");
        std::fs::remove_dir_all(temp.parent().unwrap().parent().unwrap().parent().unwrap()).ok();
    }
}
