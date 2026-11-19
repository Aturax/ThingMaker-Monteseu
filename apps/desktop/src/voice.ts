import { t } from "./i18n";

/** A dictation owns a snapshot, never the live draft. Insertion is always explicit. */
export type VoiceEvent =
  | { type: "preparing" | "transcribing" | "cancelled" }
  | { type: "recording"; seconds: number; level: number }
  | { type: "done" | "partial"; text: string }
  | { type: "error" | "warning"; message: string };
export type VoiceDownload = { received: number; total: number; phase: "downloading" | "verifying" | "ready" };
export type VoiceSnapshot = { base: string; start: number; end: number };

export function insertDictation(current: string, snapshot: VoiceSnapshot, text: string): string {
  if (!text.trim()) return current;
  if (current !== snapshot.base) return current + (current && !/\s$/.test(current) ? "\n" : "") + text;
  const start = Math.max(0, Math.min(current.length, snapshot.start));
  const end = Math.max(start, Math.min(current.length, snapshot.end));
  const before = current.slice(0, start), after = current.slice(end);
  return before + (before && !/\s$/.test(before) ? " " : "") + text + (after && !/^\s|^[.,;:!?]/.test(after) ? " " : "") + after;
}

export function voiceError(error: unknown): string {
  const message = typeof error === "object" && error && "message" in error ? String(error.message) : String(error);
  return t(message);
}
