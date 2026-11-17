import { useEffect, useRef, useState, type RefObject } from "react";
import { api } from "../ipc";
import { useStore } from "../store";
import { t } from "../i18n";
import { IS_REMOTE } from "../remote/mode";
import { insertDictation, voiceError, type VoiceSnapshot, type VoiceEvent } from "../voice";

type Result = { raw: string; text: string; edited: boolean; snapshot: VoiceSnapshot };
type Props = { target: string; draft: string; onChange: (text: string) => void; input: RefObject<HTMLTextAreaElement | null>; disabled?: boolean; localOnly?: boolean };

/** Review first: a transcript or late cleanup can never send or overwrite a live draft. */
export function VoiceInput(props: Props) {
  const [phase, setPhase] = useState("idle");
  const [seconds, setSeconds] = useState(0);
  const [level, setLevel] = useState(0);
  const [error, setError] = useState("");
  const [warning, setWarning] = useState("");
  const [needsModel, setNeedsModel] = useState(false);
  const [result, setResult] = useState<Result | null>(null);
  const [undo, setUndo] = useState<{ before: string; after: string } | null>(null);
  const run = useRef<string | null>(null);
  const latest = useRef(props); latest.current = props;
  const settings = useStore((s) => s.settings);
  const setView = useStore((s) => s.setView);
  const busy = phase !== "idle";

  const cancel = () => {
    const id = run.current; run.current = null;
    if (id) void api.voiceStop(id, true).catch(() => {});
    setPhase("idle"); setResult(null); setLevel(0); setWarning("");
  };
  useEffect(() => {
    const stop = () => { if (run.current) cancel(); };
    const hidden = () => { if (document.hidden) stop(); };
    window.addEventListener("blur", stop);
    document.addEventListener("visibilitychange", hidden);
    return () => {
      window.removeEventListener("blur", stop); document.removeEventListener("visibilitychange", hidden);
      const id = run.current; run.current = null;
      if (id) void api.voiceStop(id, true).catch(() => {});
    };
  }, [props.target]); // The native recording must not outlive its composer.

  async function start() {
    if (props.disabled || run.current) return;
    setError(""); setWarning(""); setNeedsModel(false); setResult(null); setUndo(null); setPhase("preparing");
    const id = crypto.randomUUID(); run.current = id;
    const current = latest.current;
    const snapshot = { base: current.draft, start: current.input.current?.selectionStart ?? current.draft.length, end: current.input.current?.selectionEnd ?? current.draft.length };
    const cleanup = !!settings?.voiceCleanupModel && (!props.localOnly || settings.voiceCleanupProvider === "local");
    try {
      if (!await api.voiceStatus()) { if (run.current === id) { run.current = null; setNeedsModel(true); setPhase("idle"); } return; }
      if (run.current !== id) return;
      setPhase("permission");
      await api.voicePermission();
      if (run.current !== id) return;
      setPhase("preparing");
      await api.voiceStart(id, (event: VoiceEvent) => {
        if (run.current !== id) return;
        if (event.type === "recording") { setPhase("recording"); setSeconds(event.seconds); setLevel(event.level); }
        else if (event.type === "preparing" || event.type === "transcribing") setPhase(event.type);
        else if (event.type === "cancelled") { run.current = null; setPhase("idle"); }
        else if (event.type === "warning") setWarning(t(event.message));
        else if (event.type === "error") { run.current = null; setPhase("idle"); setError(event.message); }
        else if (event.type === "done") {
          setLevel(0);
          if (!event.text.trim()) { run.current = null; setPhase("idle"); setError(t("No speech was recognized. Try again closer to the microphone.")); return; }
          setResult({ raw: event.text, text: event.text, edited: false, snapshot });
          if (!cleanup) { run.current = null; setPhase("idle"); return; }
          setPhase("cleaning");
          void api.voiceCleanup(event.text, !!props.localOnly).then((text) => {
            if (run.current !== id) return;
            if (text.trim()) setResult((r) => r && !r.edited ? { ...r, text } : r);
          }).catch(() => { if (run.current === id) setError(t("Cleanup failed. Your original dictation is available.")); })
            .finally(() => { if (run.current === id) { run.current = null; setPhase("idle"); } });
        }
      });
    } catch (e) { if (run.current === id) { run.current = null; setPhase("idle"); setError(voiceError(e)); } }
  }
  async function stop() {
    const id = run.current; if (!id) return;
    setPhase("transcribing");
    try { await api.voiceStop(id); } catch (e) { cancel(); setError(voiceError(e)); }
  }
  useEffect(() => {
    if (IS_REMOTE) return;
    const key = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.repeat || props.disabled) return;
      if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === "d") {
        event.preventDefault(); if (phase === "recording") void stop(); else if (!busy) void start();
      } else if (event.key === "Escape" && busy) { event.preventDefault(); cancel(); }
    };
    window.addEventListener("keydown", key); return () => window.removeEventListener("keydown", key);
  });

  if (IS_REMOTE) return null;
  return <div className="voice-input" aria-label={t("Voice dictation")}>
    <div className="row wrap">
      <button type="button" className={`button small ${phase === "recording" ? "voice-recording" : ""}`} disabled={props.disabled || (busy && phase !== "recording")}
        onClick={() => { if (phase === "recording") void stop(); else void start(); }} title={t("Dictate with Ctrl/Cmd+Shift+D")}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3M8 22h8"/></svg>
        {phase === "recording" ? t("Stop dictation") : t("Dictate")}
      </button>
      {busy && <button type="button" className="button small" onClick={cancel}>{t("Cancel")}</button>}
      {busy && <span className="small muted" role="status">
        {phase === "permission" ? t("Waiting for microphone permission…") : phase === "preparing" ? t("Preparing voice model…") : phase === "recording" ? t("Recording {seconds}s / 120s", { seconds: Math.floor(seconds) }) : phase === "transcribing" ? t("Transcribing on this computer…") : phase === "cleaning" ? t("Cleaning dictation…") : ""}
      </span>}
      {phase === "recording" && <meter min={0} max={1} value={Math.min(1, level * 5)} aria-label={t("Microphone level")} />}
    </div>
    {needsModel && <p className="small">{t("Download the local voice model in Settings to start dictating.")} <button type="button" className="button small" onClick={() => setView({ kind: "settings", tab: "chat" })}>{t("Settings")}</button></p>}
    {error && <p className="small" role="alert">{error}</p>}
    {warning && <p className="small" role="alert">{warning}</p>}
    {result && <div className="voice-review">
      <label className="small">{t("Review dictation before inserting")}
        <textarea className="textarea" rows={3} value={result.text} onChange={(e) => setResult({ ...result, text: e.target.value, edited: true })} />
      </label>
      <div className="row wrap">
        <button type="button" className="button small" disabled={!result.text.trim()} onClick={() => {
          const before = latest.current.draft; const after = insertDictation(before, result.snapshot, result.text);
          run.current = null; setPhase("idle"); props.onChange(after); setUndo({ before, after }); setResult(null); setWarning(""); props.input.current?.focus();
        }}>{t("Insert dictation")}</button>
        <button type="button" className="button small" onClick={() => setResult({ ...result, text: result.raw, edited: true })}>{t("Use original")}</button>
        <button type="button" className="button small" onClick={cancel}>{t("Discard")}</button>
      </div>
    </div>}
    {undo && props.draft === undo.after && <button type="button" className="button small" onClick={() => { props.onChange(undo.before); setUndo(null); }}>{t("Undo dictation insertion")}</button>}
  </div>;
}
