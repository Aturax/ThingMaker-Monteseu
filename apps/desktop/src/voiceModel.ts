import { create } from "zustand";
import { api } from "./ipc";
import { voiceError, type VoiceDownload } from "./voice";

let revision = 0;

/** Installation belongs to the app, and must remain cancellable across Settings visits. */
export const useVoiceModel = create<{
  ready: boolean;
  busy: boolean;
  progress: VoiceDownload | null;
  error: string;
  refresh: () => Promise<void>;
  download: () => Promise<void>;
  remove: () => Promise<void>;
  cancel: () => Promise<void>;
}>((set, get) => ({
  ready: false, busy: false, progress: null, error: "",
  refresh: async () => {
    if (get().busy) return;
    const observed = ++revision;
    try {
      const ready = await api.voiceStatus();
      if (observed === revision) set({ ready });
    }
    catch (e) { if (observed === revision) set({ error: voiceError(e) }); }
  },
  download: async () => {
    if (get().busy) return;
    revision++;
    set({ busy: true, error: "", progress: null });
    try {
      await api.voiceDownload((progress) => set({ progress }));
      set({ ready: true });
    } catch (e) { set({ error: voiceError(e) }); }
    finally { set({ busy: false }); }
  },
  remove: async () => {
    if (get().busy) return;
    revision++;
    set({ busy: true, progress: null, error: "" });
    try { await api.voiceRemove(); set({ ready: false }); }
    catch (e) { set({ error: voiceError(e) }); }
    finally { set({ busy: false }); }
  },
  cancel: async () => {
    try { await api.voiceCancelDownload(); }
    catch (e) { set({ error: voiceError(e) }); }
  },
}));
