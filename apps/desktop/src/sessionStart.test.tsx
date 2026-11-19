/**
 * A new session opens its chat at once: the person can write while the
 * agent starts, and what they send goes out as soon as it is ready.
 */
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const calls = vi.hoisted(() => ({ api: {} as Record<string, ReturnType<typeof vi.fn>> }));
vi.mock("./ipc", () => ({
  api: new Proxy(calls.api, { get: (target, key: string) => (target[key] ??= vi.fn(async () => undefined)) }),
}));
import { api } from "./ipc";
import { useStore } from "./store";
import type { VoiceEvent } from "./voice";
import { StartingPanel } from "./components/StartingPanel";
import { Sidebar } from "./components/Sidebar";

beforeEach(() => {
  for (const key of Object.keys(calls.api)) delete calls.api[key];
  useStore.setState({
    starting: null,
    sessions: {},
    sessionOrder: [],
    drafts: {},
    records: {},
    inspections: { w1: { trustStale: false, record: { trustState: "trusted_local" } } },
    workspaces: [{ id: "w1", displayPath: "/work/game", canonicalRoot: "/work/game" }],
    providers: [{ provider: "claude", resolved: {}, auth: { loggedIn: true } }],
    sessionDefaults: {},
    loadOdyssey: vi.fn(async () => undefined),
    loadTeam: vi.fn(async () => undefined),
    loadNesting: vi.fn(async () => undefined),
  } as never);
});
afterEach(cleanup);

describe("a new session", () => {
  it("shows its chat at once and sends what was written once it is ready", async () => {
    let ready: (value: unknown) => void = () => undefined;
    vi.mocked(api.sessionOpen).mockReturnValue(new Promise((resolve) => (ready = resolve)) as never);
    vi.mocked(api.sessionSnapshot).mockResolvedValue({ handle: { id: "s1", attachmentGeneration: "1" }, provider: "claude", agentSessionId: "agent-1", toolCalls: {}, foreground: "idle", history: [] } as never);
    vi.mocked(api.sessionHistory).mockResolvedValue([] as never);
    const send = vi.fn(async () => undefined);
    useStore.setState({ send } as never);

    const opening = useStore.getState().openSession("w1", { mode: "new" }, "claude");
    expect(useStore.getState().view).toEqual({ kind: "starting" });
    expect(useStore.getState().busy).toBeNull();
    render(<StartingPanel />);
    expect(screen.getByRole("status").textContent).toContain("Setting up the office…");
    expect(screen.getByText("Starting Claude Code…")).toBeTruthy();
    const input = screen.getByRole("textbox", { name: "Prompt" });
    fireEvent.change(input, { target: { value: "Fix the login bug" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText("Sends as soon as Claude Code is ready")).toBeTruthy();
    expect(send).not.toHaveBeenCalled();

    ready({ handle: { id: "s1", attachmentGeneration: "1" }, snapshot: {} });
    await opening;
    await waitFor(() => expect(send).toHaveBeenCalledWith("s1"));
    expect(useStore.getState().drafts.s1).toBe("Fix the login bug");
    expect(useStore.getState().starting).toBeNull();
    expect(useStore.getState().view).toEqual({ kind: "session", sessionId: "s1" });
  });

  it.each(["recording", "review"])("preserves %s dictation when the session attaches", async (stage) => {
    let ready!: (value: unknown) => void;
    let emit!: (event: VoiceEvent) => void;
    vi.mocked(api.sessionOpen).mockReturnValue(new Promise((resolve) => { ready = resolve; }) as never);
    vi.mocked(api.sessionSnapshot).mockResolvedValue({ handle: { id: "s1", attachmentGeneration: "1" }, provider: "claude", toolCalls: {}, foreground: "idle" } as never);
    vi.mocked(api.sessionHistory).mockResolvedValue([] as never);
    vi.mocked(api.voiceStatus).mockResolvedValue(true);
    vi.mocked(api.voiceStart).mockImplementation(async (_, callback) => {
      emit = callback;
      emit({ type: "recording", seconds: 0, level: 0 });
    });
    const opening = useStore.getState().openSession("w1", { mode: "new" }, "claude");
    render(<StartingPanel />);
    fireEvent.click(screen.getByRole("button", { name: "Dictate" }));
    await screen.findByRole("button", { name: "Stop dictation" });
    const done = () => emit({ type: "done", text: "my draft" });
    if (stage === "review") act(done);
    await act(async () => { ready({ handle: { id: "s1", attachmentGeneration: "1" } }); await opening; });
    expect(useStore.getState().view.kind).toBe("starting");
    expect(api.voiceStop).not.toHaveBeenCalled();
    if (stage === "recording") {
      fireEvent.click(screen.getByRole("button", { name: "Stop dictation" }));
      act(done);
    }
    fireEvent.change(screen.getByRole("textbox", { name: "Review dictation before inserting" }), { target: { value: "reviewed" } });
    fireEvent.click(screen.getByRole("button", { name: "Insert dictation" }));
    expect(useStore.getState().view).toEqual({ kind: "session", sessionId: "s1" });
    expect(useStore.getState().drafts.s1).toBe("reviewed");
    expect(api.sessionSubmit).not.toHaveBeenCalled();
  });

  it("opens from the project + immediately and keeps feedback until history is ready", async () => {
    let opened: (value: unknown) => void = () => undefined;
    let historyReady: (value: unknown) => void = () => undefined;
    vi.mocked(api.sessionOpen).mockReturnValue(new Promise((resolve) => (opened = resolve)) as never);
    vi.mocked(api.sessionSnapshot).mockResolvedValue({ handle: { id: "s1", attachmentGeneration: "1" }, provider: "claude", toolCalls: {}, foreground: "idle" } as never);
    vi.mocked(api.sessionHistory).mockReturnValue(new Promise((resolve) => (historyReady = resolve)) as never);
    function View() {
      const view = useStore((s) => s.view);
      return <><Sidebar />{view.kind === "starting" && <StartingPanel />}</>;
    }
    useStore.setState({ view: { kind: "workspace", workspaceId: "w1" } });
    render(<View />);
    fireEvent.click(screen.getByRole("button", { name: "New session" }));
    expect(screen.getByRole("status").textContent).toContain("Setting up the office…");
    fireEvent.change(screen.getByRole("textbox", { name: "Prompt" }), { target: { value: "My draft" } });
    opened({ handle: { id: "s1", attachmentGeneration: "1" } });
    await waitFor(() => expect(api.sessionHistory).toHaveBeenCalled());
    expect(useStore.getState().view).toEqual({ kind: "starting" });
    expect(screen.getByRole("textbox", { name: "Prompt" }).getAttribute("disabled")).toBeNull();
    expect(screen.getByRole("status").textContent).toContain("Setting up the office…");
    historyReady([]);
    await waitFor(() => expect(useStore.getState().view).toEqual({ kind: "session", sessionId: "s1" }));
    expect(useStore.getState().drafts.s1).toBe("My draft");
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("does not take you back to the chat if you navigate away while it starts", async () => {
    let ready: (value: unknown) => void = () => undefined;
    vi.mocked(api.sessionOpen).mockReturnValue(new Promise((resolve) => (ready = resolve)) as never);
    vi.mocked(api.sessionSnapshot).mockResolvedValue({ provider: "claude", toolCalls: {} } as never);
    vi.mocked(api.sessionHistory).mockResolvedValue([] as never);
    const opening = useStore.getState().openSession("w1", { mode: "new" }, "claude");
    useStore.getState().setView({ kind: "settings" });
    ready({ handle: { id: "s1", attachmentGeneration: "1" } });
    await opening;
    expect(useStore.getState().view).toEqual({ kind: "settings" });
    expect(useStore.getState().sessions.s1).toBeTruthy();
  });

  it("says so when the agent could not start, keeping what was written", async () => {
    vi.mocked(api.sessionOpen).mockRejectedValue({ code: "NOT_READY", message: "Claude Code is not signed in" });
    await useStore.getState().openSession("w1", { mode: "new" }, "claude");
    render(<StartingPanel />);
    expect(screen.getByText("Claude Code could not start: Claude Code is not signed in")).toBeTruthy();
  });
});

describe("returning to a conversation", () => {
  const record = { id: "r1", workspaceId: "w1", agentSessionId: "agent-1", autoTitle: "Fix the login bug", origin: "desktop", archiveState: "active", pinned: false, provider: "claude", createdAt: 0 };

  beforeEach(() => {
    useStore.setState({ records: { w1: [record] }, view: { kind: "workspace", workspaceId: "w1" } } as never);
    vi.mocked(api.sessionListOpen).mockResolvedValue([] as never);
    vi.mocked(api.sessionSnapshot).mockResolvedValue({ handle: { id: "s1", attachmentGeneration: "1" }, provider: "claude", agentSessionId: "agent-1", toolCalls: {}, foreground: "idle" } as never);
  });

  it("shows it at once, like a new one, and sends what was written once it is back", async () => {
    let ready: (value: unknown) => void = () => undefined;
    let historyReady: (value: unknown) => void = () => undefined;
    vi.mocked(api.sessionOpen).mockReturnValue(new Promise((resolve) => (ready = resolve)) as never);
    vi.mocked(api.sessionHistory).mockReturnValue(new Promise((resolve) => (historyReady = resolve)) as never);
    const send = vi.fn(async () => undefined);
    useStore.setState({ send } as never);

    const opening = useStore.getState().openSession("w1", { mode: "resume", session_id: "agent-1" });
    expect(useStore.getState().view).toEqual({ kind: "starting" });
    expect(useStore.getState().busy).toBeNull();
    render(<StartingPanel />);
    expect(screen.getByRole("region", { name: "Fix the login bug" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Recovering the conversation…");
    expect(screen.getByText("Resuming Claude Code…")).toBeTruthy();
    const input = screen.getByRole("textbox", { name: "Prompt" });
    fireEvent.change(input, { target: { value: "And the signup one" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText("Sends as soon as Claude Code is ready")).toBeTruthy();

    ready({ handle: { id: "s1", attachmentGeneration: "1" } });
    // Still waiting while its history loads, so it never looks empty.
    await waitFor(() => expect(api.sessionHistory).toHaveBeenCalled());
    expect(useStore.getState().view).toEqual({ kind: "starting" });
    historyReady([]);
    await opening;
    await waitFor(() => expect(send).toHaveBeenCalledWith("s1"));
    expect(useStore.getState().drafts.s1).toBe("And the signup one");
    expect(useStore.getState().starting).toBeNull();
    expect(useStore.getState().view).toEqual({ kind: "session", sessionId: "s1" });
  });

  it("shows the same wait while it attaches to a session still running on the computer", async () => {
    let historyReady: (value: unknown) => void = () => undefined;
    vi.mocked(api.sessionListOpen).mockResolvedValue([{ handle: { id: "s1", attachmentGeneration: "1" }, agentSessionId: "agent-1", workspaceId: "w1" }] as never);
    vi.mocked(api.sessionHistory).mockReturnValue(new Promise((resolve) => (historyReady = resolve)) as never);

    const opening = useStore.getState().openSession("w1", { mode: "resume", session_id: "agent-1" });
    expect(useStore.getState().view).toEqual({ kind: "starting" });
    await waitFor(() => expect(api.sessionHistory).toHaveBeenCalled());
    expect(useStore.getState().view).toEqual({ kind: "starting" });
    historyReady([]);
    await opening;
    expect(api.sessionOpen).not.toHaveBeenCalled();
    expect(useStore.getState().starting).toBeNull();
    expect(useStore.getState().view).toEqual({ kind: "session", sessionId: "s1" });
  });

  it("does not take you back to it if you navigate away while it loads", async () => {
    let ready: (value: unknown) => void = () => undefined;
    vi.mocked(api.sessionOpen).mockReturnValue(new Promise((resolve) => (ready = resolve)) as never);
    vi.mocked(api.sessionHistory).mockResolvedValue([] as never);
    const opening = useStore.getState().openSession("w1", { mode: "resume", session_id: "agent-1" });
    useStore.getState().setView({ kind: "settings" });
    ready({ handle: { id: "s1", attachmentGeneration: "1" } });
    await opening;
    expect(useStore.getState().view).toEqual({ kind: "settings" });
    expect(useStore.getState().starting).toBeNull();
  });

  it("says so when it could not be resumed", async () => {
    vi.mocked(api.sessionOpen).mockRejectedValue({ code: "NOT_READY", message: "Claude Code is not signed in" });
    await useStore.getState().openSession("w1", { mode: "resume", session_id: "agent-1" });
    render(<StartingPanel />);
    expect(screen.getByText("Claude Code could not resume this conversation: Claude Code is not signed in")).toBeTruthy();
  });
});
