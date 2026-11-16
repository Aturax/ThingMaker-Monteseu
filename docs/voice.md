# Desktop voice dictation

Voice dictation is optional and local: NVIDIA Parakeet TDT 0.6B v3 int8 runs on the CPU through sherpa-onnx. It does not require Python, Ollama, CUDA, an account, or a voice server. Android/Remote does not expose microphone or dictation commands.

## Try it

1. Open **Settings → Chat → Voice dictation** and download the model (465 MiB download, about 640 MB installed). The download is optional, cancellable and SHA-256 verified before installation.
2. Choose your microphone or leave the system default.
3. Optionally select **Dictation cleanup**. This is a separate provider/model choice from **Reply suggestions**, using the provider's current ThingMaker account. Off is the default. Cloud cleanup sends the transcript, never the audio. Local sessions do not use cloud cleanup.
4. In a session or the new-session composer, press **Dictate** or **Ctrl/Cmd+Shift+D**. Wait for **Recording**, speak, and press again to stop. **Esc** cancels. A recording stops at two minutes. Leaving the window or composer cancels capture.
5. Review/edit the result and press **Insert dictation**. **Use original** restores the raw transcript if cleanup changed it. The usual Send/Queue/Steer action remains a separate explicit action.

The transcript never replaces a live draft automatically. If the draft changed while speaking, insertion appends the reviewed text instead of overwriting it. A single undo button restores the pre-insertion draft while that inserted version is still unchanged. Editing the transcript prevents late cleanup from replacing it.

This first implementation transcribes **after stopping**, without live partial text. It is dictation, not always-on voice commands or a spoken assistant. No audio is saved, logged or uploaded. Model files live in the app data folder under `voice/parakeet-v3-int8`; inference resources are freed after each dictation, so the next start includes model load time.

## Architecture

- `thingmaker-voice`: bounded native CPAL capture, CPU inference, checked model acquisition, one capture at a time, scoped cancellation. A dedicated thread owns the capture stream and recognizer, keeping both off the UI and Tokio executor. The runtime is statically linked; no additional process or listener is started.
- Tauri `commands/voice.rs`: main-window-only commands and a per-invocation channel. These commands are deliberately absent from the Remote allowlist. Channel disconnection cancels recording. Native window loss of focus/close also stops capture.
- `VoiceInput`: per-composer operation identity and an independent review buffer; late results are ignored after cancellation/navigation. Settings provide microphone selection and optional cleanup.
- Cleanup shares the subscription-based, tool-free execution machinery used by reply suggestions, with its own prompt and output bound. Failure/timeout keeps the original transcript available. The local Ollama helper uses a larger output budget for dictation while retaining the old suggestion budget.

The input device's actual rate is preserved during capture. sherpa resamples it to the model's feature rate. There is no VAD filter that trims the beginning/end of a recording: the Windows investigation found an initial-phrase omission with the previous server's VAD. Exact silence returns empty, but users must still review recognition errors in noise.

## Model provenance and attribution

- Model: [NVIDIA Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), NVIDIA, licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). No endorsement by NVIDIA is implied.
- Converted/quantized ONNX int8 distribution: [sherpa-onnx NeMo model documentation](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html). Changes relative to the original: ONNX conversion and int8 quantization by the upstream distributor; ThingMaker does not modify the weights.
- Archive: `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2`, 487,211,443 bytes. SHA-256: `1495e91b59d7fc53ddca91f3431758ec99f8fa253fdb0e1e851be0f72b14b0f9`.
- Runtime: [sherpa-onnx 2.0.1](https://github.com/k2-fsa/sherpa-onnx/tree/v2.0.1), Apache-2.0, using ONNX Runtime (MIT). Native capture: CPAL 0.18.2 (Apache-2.0). Their original licenses apply to the bundled dependencies; they do not change the repository's MIT license.

Cargo uses sherpa's pinned prebuilt static runtime archives for each target. Building therefore needs network access on the first native dependency build (or `SHERPA_ONNX_ARCHIVE_DIR` pointing to matching predownloaded archives). macOS retains universal build targets; microphone purpose text and audio-input entitlement are included. Actual Mac permission/device behavior still needs validation on a Mac.

## Validation of this branch

- TypeScript typecheck; frontend/contracts suite, including independent Settings selections and draft/cancellation/late-cleanup tests.
- Native capture/control and unverified-model tests; tool-free provider contract and long-cleanup-response tests.
- Native Rust Parakeet transcribed the synthetic Spanish comparison WAV at both 16 kHz and resampled 44.1 kHz.
- The real model download, checksum, extraction and readiness path passed into an isolated scratch data directory.
- An existing contract test listing only three providers was updated to include the already-supported local provider.

An interactive microphone/permission test in the installed app and Mac validation are still needed. Tests do not establish real microphone quality. The static upstream Windows runtime emits an MSVC LIBCMT conflict warning with the current Rust toolchain; native inference and host tests run successfully despite it. Revisit runtime CRT alignment before a public release.
