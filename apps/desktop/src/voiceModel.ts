import { create } from "zustand";
import { api } from "./ipc";
import { voiceError, type VoiceDownload } from "./voice";

/** Installation belongs to the app, and must remain cancellable across Settings visits. */
export const useVoiceModel = create<{
  ready: boolean;
  busy: boolean;
  progress: VoiceDownload | null;
  error: string;
  refresh: () => Promise<void>;
  download: () => Promise<void>;
  cancel: () => Promise<void>;
}>((set, get) => ({
  ready: false, busy: false, progress: null, error: "",
  refresh: async () => {
    if (get().busy) return;
    const progress = get().progress;
    try {
      const ready = await api.voiceStatus();
      if (!get().busy && get().progress === progress) set({ ready });
    }
    catch (e) { set({ error: voiceError(e) }); }
  },
  download: async () => {
    if (get().busy) return;
    set({ busy: true, error: "", progress: null });
    try {
      await api.voiceDownload((progress) => set({ progress }));
      set({ ready: true });
    } catch (e) { set({ error: voiceError(e) }); }
    finally { set({ busy: false }); }
  },
  cancel: async () => {
    try { await api.voiceCancelDownload(); }
    catch (e) { set({ error: voiceError(e) }); }
  },
}));
