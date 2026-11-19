import { useEffect, useState } from "react";
import { PROVIDER_LABELS, type NotificationSettings, type Provider } from "@thingmaker/contracts";
import { api } from "../ipc";
import { t } from "../i18n";
import { IS_REMOTE } from "../remote/mode";
import { voiceError } from "../voice";
import { useVoiceModel } from "../voiceModel";
import { Select, SettingGroup, SettingRow, type Choice } from "./settings/controls";

export function VoiceSettings({ settings, update, modelChoices }: { settings: NotificationSettings; update: (patch: Partial<NotificationSettings>) => void; modelChoices: Choice<string>[] }) {
  const { ready, busy, progress, error, refresh, download, remove, cancel } = useVoiceModel();
  const [devices, setDevices] = useState<{ id: string; name: string }[]>([]);
  const [deviceError, setDeviceError] = useState("");
  useEffect(() => {
    if (IS_REMOTE) return;
    let live = true;
    void refresh();
    void Promise.resolve().then(() => api.voiceDevices()).then((value) => { if (live) setDevices(value); }).catch((e) => { if (live) setDeviceError(voiceError(e)); });
    return () => { live = false; };
  }, []);
  if (IS_REMOTE) return null;
  const cleanup = settings.voiceCleanupModel && settings.voiceCleanupProvider ? `${settings.voiceCleanupProvider}::${settings.voiceCleanupModel}` : "";
  const choices = [...modelChoices];
  if (cleanup && settings.voiceCleanupProvider && !choices.some((c) => c.value === cleanup)) choices.push({ value: cleanup, label: `${PROVIDER_LABELS[settings.voiceCleanupProvider]} · ${settings.voiceCleanupModel}` });
  const inputs = [{ value: "", label: t("System default microphone") }, ...devices.map((d) => ({ value: d.id, label: d.name }))];
  if (settings.voiceInputDevice && !inputs.some((d) => d.value === settings.voiceInputDevice)) inputs.push({ value: settings.voiceInputDevice, label: t("Saved microphone (not connected)") });
  return <SettingGroup>
    <SettingRow name={t("Voice dictation")} hint={t("Parakeet v3 · CPU · Spanish and English detected automatically")}
      info={t("Audio stays on this computer. Provisional text updates about every 2 seconds. Stop for a final transcription, then review and insert. Maximum 2 minutes. Ctrl/Cmd+Shift+D starts or stops; Esc cancels.")}>
      {ready ? <span className="small">{t("Voice model ready")}</span> : <button type="button" className="button small" disabled={busy} onClick={() => void download()}>{t("Download voice model (465 MiB)")}</button>}
      {ready && <button type="button" className="button small" disabled={busy} onClick={() => void remove()}>{t("Remove voice model")}</button>}
      {busy && !ready && <><span className="small" role="status">{progress?.phase === "verifying" ? t("Verifying voice model…") : t("Downloading voice model: {percent}%", { percent: Math.floor(100 * (progress?.received ?? 0) / (progress?.total || 1)) })}</span><button type="button" className="button small" onClick={() => void cancel()}>{t("Cancel")}</button></>}
      {error && <span className="small" role="alert">{error}</span>}
    </SettingRow>
    <SettingRow name={t("Microphone")}>
      <Select label={t("Microphone")} choices={inputs} value={settings.voiceInputDevice ?? ""} onChange={(voiceInputDevice) => update({ voiceInputDevice })} />
      <button type="button" className="button small" onClick={() => {
        setDeviceError("");
        void api.voiceDevices().then(setDevices).catch((e) => setDeviceError(voiceError(e)));
      }}>{t("Refresh microphones")}</button>
      {deviceError && <span className="small" role="alert">{deviceError}</span>}
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
