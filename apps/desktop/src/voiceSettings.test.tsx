import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { NotificationSettings } from "@thingmaker/contracts";
import { VoiceSettings } from "./components/VoiceSettings";
import { api } from "./ipc";
import { useVoiceModel } from "./voiceModel";
import type { VoiceDownload } from "./voice";

beforeEach(() => {
  useVoiceModel.setState({ ready: false, busy: false, progress: null, error: "" });
  vi.spyOn(api, "voiceStatus").mockResolvedValue(false);
  vi.spyOn(api, "voiceDevices").mockResolvedValue([]);
  vi.spyOn(api, "voiceCancelDownload").mockResolvedValue();
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
const view = () => <VoiceSettings settings={{} as NotificationSettings} update={() => {}} modelChoices={[]} />;

it("keeps download progress and cancellation when Settings unmounts and returns", async () => {
  let progress!: (p: VoiceDownload) => void;
  let finish!: () => void;
  vi.spyOn(api, "voiceDownload").mockImplementation((onProgress) => {
    progress = onProgress;
    return new Promise((resolve) => { finish = resolve; });
  });
  const first = render(view());
  await act(async () => {});
  fireEvent.click(screen.getByRole("button", { name: "Download voice model (465 MiB)" }));
  act(() => progress({ received: 50, total: 100, phase: "downloading" }));
  first.unmount();
  render(view());
  await act(async () => {});
  expect(screen.getByRole("status").textContent).toContain("50%");
  expect((screen.getByRole("button", { name: "Download voice model (465 MiB)" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(api.voiceCancelDownload).toHaveBeenCalledOnce();
  await act(async () => finish());
  expect(screen.getByText("Voice model ready")).toBeTruthy();
  expect(api.voiceDownload).toHaveBeenCalledOnce();
});
it("shows input enumeration errors and refreshes newly connected microphones", async () => {
  vi.mocked(api.voiceDevices).mockRejectedValueOnce({ message: "Audio device unavailable" })
    .mockResolvedValueOnce([{ id: "built-in", name: "MacBook Pro Microphone" }]);
  render(view());
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Audio device unavailable"));
  fireEvent.click(screen.getByRole("button", { name: "Refresh microphones" }));
  await waitFor(() => expect(api.voiceDevices).toHaveBeenCalledTimes(2));
  fireEvent.click(screen.getByRole("button", { name: "Microphone" }));
  expect(screen.getByRole("option", { name: "MacBook Pro Microphone" })).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
});
it("can remove the installed voice model and offer a fresh download", async () => {
  vi.mocked(api.voiceStatus).mockResolvedValue(true);
  vi.spyOn(api, "voiceRemove").mockResolvedValue();
  render(view());
  fireEvent.click(await screen.findByRole("button", { name: "Remove voice model" }));
  await screen.findByRole("button", { name: "Download voice model (465 MiB)" });
  expect(api.voiceRemove).toHaveBeenCalledOnce();
});
