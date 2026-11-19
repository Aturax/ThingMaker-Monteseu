# Desktop voice dictation

Voice dictation is optional and local: NVIDIA Parakeet TDT 0.6B v3 int8 runs on the CPU through sherpa-onnx. It does not require Python, Ollama, CUDA, an account, or a voice server. Android/Remote does not expose microphone or dictation commands.

## Try it

1. Open **Settings → Chat → Voice dictation** and download the model (465 MiB download, about 640 MB installed). The download requires 1.2 GB of free space and is cancellable and SHA-256 verified before installation. Use **Remove voice model** here to reclaim its installed space.
2. Choose your microphone or leave the system default.
   **Refresh microphones** reloads connected inputs without restarting the app. Device-enumeration errors are shown instead of silently hiding the inputs.
3. Optionally select **Dictation cleanup**. This is a separate provider/model choice from **Reply suggestions**, using the provider's current ThingMaker account. Off is the default. Cloud cleanup sends the transcript, never the audio. Local sessions do not use cloud cleanup.
4. In a session or the new-session composer, press **Dictate** or **Ctrl/Cmd+Shift+D**. Wait for **Recording**, speak, and press again to stop. **Esc** cancels. A recording stops at two minutes. Switching windows while the microphone is open cancels capture. After you stop, transcription and cleanup continue when you switch windows. Leaving the composer discards the take.
   On macOS, the app explicitly requests microphone consent before opening CoreAudio. If the permission dialog cancels dictation by taking focus, return to the app and press **Dictate** again. Denied access gives directions to System Settings. Granting permission after cancellation never starts a recording in the background.
5. After about two seconds of recording, provisional text starts appearing in the review box and refreshes approximately every two seconds. It is read-only while recording and during the final pass. Stop recording to obtain the final transcription, review/edit it and press **Insert dictation**. **Use original** restores the raw transcript if cleanup changed it. The usual Send/Queue/Steer action remains a separate explicit action.

If the audio host reports an interruption (Xrun), recording continues and a warning remains visible through transcript review, including if cleanup fails. The transcript may be incomplete: review it carefully or record again. An actual local buffer overflow, device disconnection or stream failure aborts the take with a distinct error; incomplete queued audio is never silently overwritten.

The transcript never replaces a live draft automatically. If the draft changed while speaking, insertion appends the reviewed text instead of overwriting it. A single undo button restores the pre-insertion draft while that inserted version is still unchanged. Editing the transcript prevents late cleanup from replacing it.

Model installation progress and cancellation remain available when leaving and returning to Settings.

The local offline model generates provisional previews while you speak and independently re-transcribes the full recording after stopping. Previews can revise earlier words and provisional window boundaries; the final result replaces them. The two-second cadence is a target: a slower computer skips preview requests rather than building up a queue. It is dictation, not always-on voice commands or a spoken assistant. No audio is saved, logged or uploaded. Model files live in the app data folder under `voice/parakeet-v3-int8`; inference resources are freed after each dictation, so the next start includes model load time.

## Architecture

- `thingmaker-voice`: bounded native CPAL capture, CPU inference, checked model acquisition, one capture at a time, scoped cancellation. A capture thread owns the stream; a separate inference thread exclusively owns one recognizer for both previews and the final pass. Both stay off the UI and Tokio executor. Previews process at most 16 seconds of overlapping audio, keep earlier text outside that window, and allow at most one preview in flight. The final pass uses the complete bounded recording and bypasses provisional stitching. Stopping suppresses late previews; cancellation shuts down and joins the inference thread. The real-time callback writes downmixed samples into a preallocated, lock-free rtrb ring; it does not allocate blocks or use a mutex-backed channel. The ring holds one second at the device rate, and buffers are allocated before capture starts. Non-audio diagnostics record callback/frame counts, maximum callback duration and error/overrun codes, never audio or transcript content. The runtime is statically linked; no additional process or listener is started.
- Tauri `commands/voice.rs`: main-window-only commands and a per-invocation channel. These commands are deliberately absent from the Remote allowlist. Channel disconnection cancels recording. Native window loss of focus/close also stops capture.
- `VoiceInput`: per-composer operation identity and an independent review buffer; late results are ignored after cancellation/navigation. Settings provide microphone selection and optional cleanup.
- Cleanup shares the subscription-based, tool-free execution machinery used by reply suggestions, with its own prompt and output bound. Failure/timeout keeps the original transcript available. The local Ollama helper uses a larger output budget for dictation while retaining the old suggestion budget.

The input device's actual rate is preserved during capture. sherpa resamples it to the model's feature rate. There is no VAD filter that trims the beginning/end of a recording: the Windows investigation found an initial-phrase omission with the previous server's VAD. Exact silence returns empty, but users must still review recognition errors in noise.

## Model provenance and attribution

- Model: [NVIDIA Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), NVIDIA, licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). No endorsement by NVIDIA is implied.
- Converted/quantized ONNX int8 distribution: [sherpa-onnx NeMo model documentation](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html). Changes relative to the original: ONNX conversion and int8 quantization by the upstream distributor; ThingMaker does not modify the weights.
- Archive: `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2`, 487,211,443 bytes. SHA-256: `1495e91b59d7fc53ddca91f3431758ec99f8fa253fdb0e1e851be0f72b14b0f9`.
- Runtime: [sherpa-onnx 2.0.1](https://github.com/k2-fsa/sherpa-onnx/tree/v2.0.1), Apache-2.0, using ONNX Runtime (MIT). Native capture: CPAL 0.18.2 (Apache-2.0). Their original licenses apply to the bundled dependencies; they do not change the repository's MIT license.

Release builds download and verify sherpa’s target-specific static archives against the SHA-256 values in `scripts/fetch-sherpa.py` before invoking Cargo. Building therefore needs network access on the first native dependency build (or `SHERPA_ONNX_ARCHIVE_DIR` pointing to matching predownloaded archives). macOS retains universal build targets; microphone purpose text and audio-input entitlement are included. Linux source builds require ALSA development headers (`libasound2-dev` on Debian and Ubuntu). After an ad-hoc-signed macOS update, you may need to grant microphone access again. Mac permission/device checks and their limits are recorded in the validation reports.

## Validation of this branch

- TypeScript typecheck; frontend/contracts suite, including independent Settings selections and draft/cancellation/late-cleanup tests.
- Native capture/control and unverified-model tests; tool-free provider contract and long-cleanup-response tests.
- Native Rust Parakeet transcribed the synthetic Spanish comparison WAV at both 16 kHz and resampled 44.1 kHz.
- The real model download, checksum, extraction and readiness path passed into an isolated scratch data directory.
- An existing contract test listing only three providers was updated to include the already-supported local provider.

See the [macOS validation report](research/voice-validation-mac-2026-11-17.md) for the checks performed on Apple Silicon and the remaining interactive/Intel checks. Tests do not establish real microphone quality. Windows release builds target `x86_64-pc-windows-msvc` explicitly with `STATIC_VCRUNTIME=false` (disables Tauri’s dynamic UCRT override) and `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS=-Ctarget-feature=+crt-static` so Rust and sherpa's static-MT runtime use the same C runtime. Apply these settings to manual Windows builds as well. Release CI records the size of each NSIS installer and universal macOS DMG in its build summary.

Live-preview implementation and Mac cadence checks: [2026-11-17 validation](research/voice-live-mac-2026-11-17.md).
