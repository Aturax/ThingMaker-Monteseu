import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { VoiceInput } from "./components/VoiceInput";
import { useLanguageForTests } from "./i18n";
import { api } from "./ipc";
import { useStore } from "./store";
import { insertDictation, type VoiceEvent } from "./voice";
import type { NotificationSettings } from "@thingmaker/contracts";

let emit: (e: VoiceEvent) => void;
beforeEach(() => {
  vi.spyOn(api, "voiceStatus").mockResolvedValue(true);
  vi.spyOn(api, "voicePermission").mockResolvedValue();
  vi.spyOn(api, "voiceStart").mockImplementation(async (_, cb) => { emit = cb; cb({ type: "recording", seconds: 0, level: 0 }); });
  vi.spyOn(api, "voiceStop").mockResolvedValue();
  useStore.setState({ settings: {} as NotificationSettings });
});
afterEach(() => { cleanup(); useLanguageForTests("en"); vi.restoreAllMocks(); });
function Composer({ localOnly = false, disabled = false }: { localOnly?: boolean; disabled?: boolean }) {
  const [text, setText] = useState("existing draft"); const input = useRef<HTMLTextAreaElement>(null);
  return <><textarea aria-label="Composer" ref={input} value={text} onChange={(e) => setText(e.target.value)} /><VoiceInput target="session" draft={text} input={input} onChange={setText} localOnly={localOnly} disabled={disabled} /></>;
}
async function start() { fireEvent.click(screen.getByRole("button", { name: "Dictate" })); await waitFor(() => expect(api.voiceStart).toHaveBeenCalled()); }
it("inserts only after review and never overwrites edits made while recording", async () => {
  render(<Composer />); await start();
  fireEvent.change(screen.getByRole("textbox", { name: "Composer" }), { target: { value: "changed meanwhile" } });
  act(() => emit({ type: "done", text: "dictated words" }));
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toBe("changed meanwhile");
  fireEvent.click(screen.getByRole("button", { name: "Insert dictation" }));
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toBe("changed meanwhile\ndictated words");
  fireEvent.click(screen.getByRole("button", { name: "Undo dictation insertion" }));
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toBe("changed meanwhile");
});
it("ignores a native result after cancellation", async () => {
  render(<Composer />); await start(); fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  act(() => emit({ type: "done", text: "stale transcript" }));
  expect(screen.queryByText("Review dictation before inserting")).toBeNull();
  expect(api.voiceStop).toHaveBeenCalledWith(expect.any(String), true);
});
it("late cleanup cannot replace a user-edited transcript", async () => {
  let finish!: (s: string) => void;
  vi.spyOn(api, "voiceCleanup").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  useStore.setState({ settings: { voiceCleanupModel: "chosen", voiceCleanupProvider: "codex" } as NotificationSettings });
  render(<Composer />); await start(); act(() => emit({ type: "done", text: "raw text" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Review dictation before inserting" }), { target: { value: "my correction" } });
  await act(async () => finish("late rewrite"));
  expect((screen.getByRole("textbox", { name: "Review dictation before inserting" }) as HTMLTextAreaElement).value).toBe("my correction");
  fireEvent.click(screen.getByRole("button", { name: "Use original" }));
  expect((screen.getByRole("textbox", { name: "Review dictation before inserting" }) as HTMLTextAreaElement).value).toBe("raw text");
});
it("keeps the original available when cleanup fails", async () => {
  vi.spyOn(api, "voiceCleanup").mockRejectedValue(new Error("provider unavailable"));
  useStore.setState({ settings: { voiceCleanupModel: "chosen", voiceCleanupProvider: "codex" } as NotificationSettings });
  render(<Composer />); await start();
  act(() => emit({ type: "done", text: "Do not change the limit of 15." }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Cleanup failed"));
  expect((screen.getByRole("textbox", { name: "Review dictation before inserting" }) as HTMLTextAreaElement).value).toBe("Do not change the limit of 15.");
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toBe("existing draft");
});
it("keeps an audio interruption warning visible through recording, review and failed cleanup", async () => {
  vi.spyOn(api, "voiceCleanup").mockRejectedValue(new Error("provider unavailable"));
  useStore.setState({ settings: { voiceCleanupModel: "chosen", voiceCleanupProvider: "codex" } as NotificationSettings });
  render(<Composer />); await start();
  const warning = "The microphone reported an audio interruption. Review the dictation carefully or record it again.";
  act(() => emit({ type: "warning", message: warning }));
  expect(screen.getByText(warning)).toBeTruthy();
  expect(screen.getByRole("button", { name: "Stop dictation" })).toBeTruthy();
  act(() => emit({ type: "done", text: "Check this transcript" }));
  await waitFor(() => expect(screen.getByText("Cleanup failed. Your original dictation is available.")).toBeTruthy());
  expect(screen.getByText(warning)).toBeTruthy();
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toBe("existing draft");
  fireEvent.click(screen.getByRole("button", { name: "Discard" }));
  expect(screen.queryByText(warning)).toBeNull();
});
it("never cleans a local session through a cloud provider", async () => {
  const clean = vi.spyOn(api, "voiceCleanup");
  useStore.setState({ settings: { voiceCleanupModel: "chosen", voiceCleanupProvider: "codex" } as NotificationSettings });
  render(<Composer localOnly />); await start(); act(() => emit({ type: "done", text: "private text" }));
  expect(clean).not.toHaveBeenCalled();
});
it("allows stopping when the composer detaches mid-recording", async () => {
  const view = render(<Composer />); await start();
  view.rerender(<Composer disabled />);
  fireEvent.click(screen.getByRole("button", { name: "Stop dictation" }));
  expect(api.voiceStop).toHaveBeenCalledWith(expect.any(String));
  act(() => emit({ type: "done", text: "retained" }));
  expect(screen.getByDisplayValue("retained")).toBeTruthy();
});
it("keeps the original when settings have a model without a cleanup provider", async () => {
  const clean = vi.spyOn(api, "voiceCleanup");
  useStore.setState({ settings: { voiceCleanupModel: "orphan" } as NotificationSettings });
  render(<Composer />); await start(); act(() => emit({ type: "done", text: "raw" }));
  expect(clean).not.toHaveBeenCalled();
  expect(screen.getByDisplayValue("raw")).toBeTruthy();
});
it("translates native dictation errors when the UI is Spanish", async () => {
  render(<Composer />); await start();
  useLanguageForTests("es");
  act(() => emit({ type: "error", message: "The microphone was disconnected. Reconnect it and record again." }));
  expect(screen.getByRole("alert").textContent).toContain("El micrófono se ha desconectado");
});
it("cancels native capture when its composer unmounts", async () => {
  const view = render(<Composer />); await start(); view.unmount();
  expect(api.voiceStop).toHaveBeenCalledWith(expect.any(String), true);
});
it("never opens a microphone after cancellation while macOS permission is pending", async () => {
  let grant!: () => void;
  vi.mocked(api.voicePermission).mockImplementation(() => new Promise((resolve) => { grant = resolve; }));
  render(<Composer />);
  fireEvent.click(screen.getByRole("button", { name: "Dictate" }));
  await waitFor(() => expect(api.voicePermission).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await act(async () => grant());
  expect(api.voiceStart).not.toHaveBeenCalled();
});
it("shows permission denial without opening a microphone and allows retry", async () => {
  vi.mocked(api.voicePermission).mockRejectedValueOnce({ message: "Microphone access is disabled." });
  render(<Composer />);
  fireEvent.click(screen.getByRole("button", { name: "Dictate" }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Microphone access is disabled."));
  expect(api.voiceStart).not.toHaveBeenCalled();
  await start();
});
it("uses the Mac shortcut to stop and Escape to cancel without touching the draft", async () => {
  render(<Composer />);
  fireEvent.keyDown(window, { key: "D", metaKey: true, shiftKey: true });
  await waitFor(() => expect(api.voiceStart).toHaveBeenCalled());
  fireEvent.keyDown(window, { key: "D", metaKey: true, shiftKey: true });
  expect(api.voiceStop).toHaveBeenLastCalledWith(expect.any(String));
  fireEvent.keyDown(window, { key: "Escape" });
  act(() => emit({ type: "done", text: "stale result" }));
  expect(api.voiceStop).toHaveBeenLastCalledWith(expect.any(String), true);
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toBe("existing draft");
});
it("cancels on loss of focus and ignores later native events", async () => {
  render(<Composer />); await start();
  fireEvent(window, new Event("blur"));
  act(() => emit({ type: "done", text: "stale result" }));
  expect(screen.queryByText("Review dictation before inserting")).toBeNull();
  expect(api.voiceStop).toHaveBeenCalledWith(expect.any(String), true);
});
it("preserves a stopped take across blur during transcription and cleanup", async () => {
  let finish!: (text: string) => void;
  vi.spyOn(api, "voiceCleanup").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  useStore.setState({ settings: { voiceCleanupModel: "chosen", voiceCleanupProvider: "codex" } as NotificationSettings });
  render(<Composer />); await start();
  fireEvent.click(screen.getByRole("button", { name: "Stop dictation" }));
  fireEvent(window, new Event("blur"));
  act(() => emit({ type: "done", text: "raw" }));
  fireEvent(window, new Event("blur"));
  await act(async () => finish("cleaned"));
  expect(screen.getByDisplayValue("cleaned")).toBeTruthy();
  expect(api.voiceStop).toHaveBeenCalledTimes(1);
});
it("honours the captured selection only while the draft is unchanged", () => {
  expect(insertDictation("hello world", { base: "hello world", start: 6, end: 11 }, "friend")).toBe("hello friend");
  expect(insertDictation("edited", { base: "hello world", start: 6, end: 11 }, "friend")).toBe("edited\nfriend");
});
it("rewrites provisional review text, protects the draft and waits for the final pass before insertion", async () => {
  render(<Composer />); await start();
  act(() => emit({ type: "partial", text: "Set the limit to fifteen" }));
  const review = () => screen.getByRole("textbox", { name: "Review dictation before inserting" }) as HTMLTextAreaElement;
  expect(review().value).toBe("Set the limit to fifteen");
  expect(review().readOnly).toBe(true);
  expect((screen.getByRole("button", { name: "Insert dictation" }) as HTMLButtonElement).disabled).toBe(true);
  act(() => emit({ type: "partial", text: "Set the limit to fifty" }));
  expect(review().value).toBe("Set the limit to fifty");
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toBe("existing draft");
  fireEvent.click(screen.getByRole("button", { name: "Stop dictation" }));
  act(() => { emit({ type: "partial", text: "late preview" }); emit({ type: "recording", seconds: 7, level: 0.1 }); });
  expect(review().value).toBe("Set the limit to fifty");
  expect(screen.getByText("Transcribing on this computer…")).toBeTruthy();
  act(() => emit({ type: "done", text: "Do not change the limit yet." }));
  expect(review().value).toBe("Do not change the limit yet.");
  expect(review().readOnly).toBe(false);
  fireEvent.change(review(), { target: { value: "Reviewed final text" } });
  fireEvent.click(screen.getByRole("button", { name: "Insert dictation" }));
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toContain("Reviewed final text");
});
it("never sends provisional text to cleanup and clears it when the final result is empty", async () => {
  const clean = vi.spyOn(api, "voiceCleanup");
  useStore.setState({ settings: { voiceCleanupModel: "chosen", voiceCleanupProvider: "codex" } as NotificationSettings });
  render(<Composer />); await start();
  act(() => emit({ type: "partial", text: "Possible speech" }));
  expect(clean).not.toHaveBeenCalled();
  act(() => emit({ type: "done", text: "" }));
  expect(screen.queryByRole("textbox", { name: "Review dictation before inserting" })).toBeNull();
  expect(screen.getByRole("alert").textContent).toContain("No speech was recognized");
});
it("discards provisional results on cancellation and ignores late updates", async () => {
  render(<Composer />); await start();
  act(() => emit({ type: "partial", text: "Provisional words" }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  act(() => { emit({ type: "partial", text: "late" }); emit({ type: "done", text: "late final" }); });
  expect(screen.queryByRole("textbox", { name: "Review dictation before inserting" })).toBeNull();
  expect((screen.getByRole("textbox", { name: "Composer" }) as HTMLTextAreaElement).value).toBe("existing draft");
});
