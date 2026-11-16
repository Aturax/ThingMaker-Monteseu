import { useEffect, useState } from "react";
import { PROVIDER_LABELS, type NotificationSettings, type Provider } from "@thingmaker/contracts";
import { api } from "../ipc";
import { t } from "../i18n";
import { IS_REMOTE } from "../remote/mode";
import { voiceError, type VoiceDownload } from "../voice";
import { Select, SettingGroup, SettingRow, type Choice } from "./settings/controls";

export function VoiceSettings({ settings, update, modelChoices }: { settings: NotificationSettings; update: (patch: Partial<NotificationSettings>) => void; modelChoices: Choice<string>[] }) {
  const [ready, setReady] = useState(false);
  const [devices, setDevices] = useState<{ id: string; name: string }[]>([]);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<VoiceDownload | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    if (IS_REMOTE) return;
    let live = true;
    void Promise.resolve().then(() => api.voiceStatus()).then((value) => { if (live) setReady(value); }).catch(() => {});
    void Promise.resolve().then(() => api.voiceDevices()).then((value) => { if (live) setDevices(value); }).catch(() => {});
    return () => { live = false; };
  }, []);
  if (IS_REMOTE) return null;
  const cleanup = settings.voiceCleanupModel ? `${settings.voiceCleanupProvider ?? "claude"}::${settings.voiceCleanupModel}` : "";
  const choices = [...modelChoices];
  if (cleanup && !choices.some((c) => c.value === cleanup)) choices.push({ value: cleanup, label: `${PROVIDER_LABELS[settings.voiceCleanupProvider ?? "claude"]} · ${settings.voiceCleanupModel}` });
  const inputs = [{ value: "", label: t("System default microphone") }, ...devices.map((d) => ({ value: d.id, label: d.name }))];
  if (settings.voiceInputDevice && !inputs.some((d) => d.value === settings.voiceInputDevice)) inputs.push({ value: settings.voiceInputDevice, label: t("Saved microphone (not connected)") });
  return <SettingGroup>
    <SettingRow name={t("Voice dictation")} hint={t("Parakeet v3 · CPU · Spanish and English detected automatically")}
      info={t("Audio stays on this computer. Stop recording to transcribe, review the text, then insert it. Maximum 2 minutes. Ctrl/Cmd+Shift+D starts or stops; Esc cancels.")}>
      {ready ? <span className="small">{t("Voice model ready")}</span> : <button type="button" className="button small" disabled={busy} onClick={() => {
        setBusy(true); setError(""); setProgress(null);
        void api.voiceDownload(setProgress).then(() => setReady(true)).catch((e) => setError(voiceError(e))).finally(() => setBusy(false));
      }}>{t("Download voice model (465 MiB)")}</button>}
      {busy && <><span className="small" role="status">{progress?.phase === "verifying" ? t("Verifying voice model…") : t("Downloading voice model: {percent}%", { percent: Math.floor(100 * (progress?.received ?? 0) / (progress?.total || 1)) })}</span><button type="button" className="button small" onClick={() => void api.voiceCancelDownload().catch((e) => setError(voiceError(e)))}>{t("Cancel")}</button></>}
      {error && <span className="small" role="alert">{error}</span>}
    </SettingRow>
    <SettingRow name={t("Microphone")}>
      <Select label={t("Microphone")} choices={inputs} value={settings.voiceInputDevice ?? ""} onChange={(voiceInputDevice) => update({ voiceInputDevice })} />
    </SettingRow>
    <SettingRow name={t("Dictation cleanup")} hint={t("Optional; independent of reply suggestions")}
      info={t("The selected provider receives only the transcript, using its current account. Cloud providers send that text off this computer. The original stays available; cleanup never sends your message.")}>
      <Select choices={choices} label={t("Dictation cleanup")} value={cleanup} onChange={(value) => {
        if (!value) { update({ voiceCleanupModel: "" }); return; }
        const [provider, ...model] = value.split("::"); update({ voiceCleanupProvider: provider as Provider, voiceCleanupModel: model.join("::") });
      }} />
    </SettingRow>
    <p className="small muted">{t("Parakeet weights by NVIDIA (CC BY 4.0); int8 conversion and runtime by sherpa-onnx. About 640 MB on disk. The model is released from memory after each dictation.")}</p>
  </SettingGroup>;
}
