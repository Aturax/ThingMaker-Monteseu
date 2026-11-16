import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { VoiceInput } from "./components/VoiceInput";
import { api } from "./ipc";
import { useStore } from "./store";
import { insertDictation, type VoiceEvent } from "./voice";
import type { NotificationSettings } from "@thingmaker/contracts";

let emit: (e: VoiceEvent) => void;
beforeEach(() => {
  vi.spyOn(api, "voiceStatus").mockResolvedValue(true);
  vi.spyOn(api, "voiceStart").mockImplementation(async (_, cb) => { emit = cb; cb({ type: "recording", seconds: 0, level: 0 }); });
  vi.spyOn(api, "voiceStop").mockResolvedValue();
  useStore.setState({ settings: {} as NotificationSettings });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
function Composer({ localOnly = false }: { localOnly?: boolean }) {
  const [text, setText] = useState("existing draft"); const input = useRef<HTMLTextAreaElement>(null);
  return <><textarea aria-label="Composer" ref={input} value={text} onChange={(e) => setText(e.target.value)} /><VoiceInput target="session" draft={text} input={input} onChange={setText} localOnly={localOnly} /></>;
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
it("never cleans a local session through a cloud provider", async () => {
  const clean = vi.spyOn(api, "voiceCleanup");
  useStore.setState({ settings: { voiceCleanupModel: "chosen", voiceCleanupProvider: "codex" } as NotificationSettings });
  render(<Composer localOnly />); await start(); act(() => emit({ type: "done", text: "private text" }));
  expect(clean).not.toHaveBeenCalled();
});
it("cancels native capture when its composer unmounts", async () => {
  const view = render(<Composer />); await start(); view.unmount();
  expect(api.voiceStop).toHaveBeenCalledWith(expect.any(String), true);
});
it("honours the captured selection only while the draft is unchanged", () => {
  expect(insertDictation("hello world", { base: "hello world", start: 6, end: 11 }, "friend")).toBe("hello friend");
  expect(insertDictation("edited", { base: "hello world", start: 6, end: 11 }, "friend")).toBe("edited\nfriend");
});
