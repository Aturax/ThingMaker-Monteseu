# Encargo: modo dictado por voz en ThingMaker (escritorio)

Documento para un agente que no ha visto la conversación en la que se preparó. Fecha: 2026-11-16.

## Qué se pide

Investigar **cómo añadir un modo dictado por voz a la app de escritorio de ThingMaker** (`D:\repos\thingmaker`, Tauri 2 en macOS y Windows) y proponer un diseño. **No implementarlo.**

**El reconocimiento de voz (STT) corre en el ordenador**, dentro de la app de escritorio. Hay dos flujos, por orden de prioridad:

1. **Modo dictado en el ordenador (lo importante).** El usuario activa el dictado en el escritorio con un botón o un atajo y habla al micrófono del ordenador. La transcripción aparece en el compositor de la sesión, donde puede editarla antes de enviar. Lo ideal es ver el texto mientras habla.
2. **Dictado desde el móvil (secundario).** La app ThingMaker Remote **solo graba** y manda el **audio** al ordenador por el puente del móvil que ya existe. El ordenador lo transcribe y **devuelve el texto al móvil**, que lo pone en su compositor para editarlo antes de enviarlo. El móvil no transcribe nada.

**Entregable:** un informe en `D:\repos\thingmaker\docs\plans\voice.md` con:
1. Diseño del modo dictado de escritorio de punta a punta: captura de micrófono, motor STT, streaming de resultados parciales, interfaz y atajo, y limpieza (si hace falta).
2. Opciones técnicas para cada pieza, con pros y contras, y una recomendación justificada para **macOS y Windows**.
3. Diseño del flujo del móvil sobre el puente existente: comandos nuevos, transporte del audio y respuesta con el texto.
4. Cómo encaja con los principios y ADRs de ThingMaker, con los conflictos explícitos.
5. Cambios concretos por módulo (ficheros y componentes), riesgos, esfuerzo, fases sugeridas y plan de pruebas.
6. Preguntas que solo puede decidir el usuario.

**Reglas:**
- No modificar código ni hacer commits en este repo ni en `D:\tts-stt`. Solo leer, investigar y escribir el informe.
- Separar lo **verificado** (con ruta de fichero o URL) de lo **inferido**.
- Para versiones de librerías y APIs posteriores a tu fecha de corte, comprueba en la web; no asumas.

## Contexto 1: ThingMaker (lo verificado al preparar el encargo; amplíalo)

**Qué es.** Escritorio Tauri 2: React + TypeScript en `apps/desktop/src`, el host de Tauri en `apps/desktop/src-tauri/src` y el motor en Rust en `crates/thingmaker-supervisor`.
- Maneja Claude Code, Codex y Gemini a través de sus programas oficiales.
- Corre en macOS (Apple silicon) y Windows x64.
- Es un repo público con licencia MIT (`Monteseu/ThingMaker`). El usuario es colaborador (hizo el soporte de Windows), así que **el diseño tiene que ser aceptable para los mantenedores**: genérico, opcional y sin depender de la infraestructura personal del usuario.

**Principios que condicionan el diseño** (`docs/adr/`):
- **ADR-009, "Subscriptions only. No API keys, ever."** La app nunca maneja claves de proveedores. Una limpieza del texto con una API key (como hace hoy el proyecto de voz del usuario con Gemini) **choca** con esto.
- **ADR-001** (interfaz web en Tauri), **ADR-003** (el trabajo privilegiado lo hace Rust), **ADR-004** (ACP local sin puerto de red).
- **ADR-012** (un único sistema de móvil con dos vías de acceso: relay de Cloudflare o directo por túnel propio).
- **Local Models** (`docs/local-models.md`): Ollama como proveedor "gratis y privado". Un STT local encaja con ese espíritu.

**Puntos de entrada probables:**
- **Compositor de la sesión:** localízalo. Los candidatos son `apps/desktop/src/components/PendingPrompt.tsx` y `SessionPanel.tsx`.
- **Ajustes:** `apps/desktop/src/components/SettingsPanel.tsx`, con i18n en inglés y español en `apps/desktop/src/i18n/`.
- **Sugerencias de respuesta** (`apps/desktop/src/replySuggestion.ts`; en el supervisor, `agents/local/ollama.rs` y `agents/claude/launch.rs`). Ya envía una petición aparte a un proveedor elegido (Claude, Codex, Antigravity u Ollama) **sin API keys**. Es candidata para hacer la limpieza del dictado respetando ADR-009. **Compruébalo.**
- **Puente del móvil en el host:** `apps/desktop/src-tauri/src/remote/`:
  - `dispatch.rs`: la lista de comandos.
  - `relay.rs` y `server.rs`: las dos vías de conexión.
  - `images.rs`: los adjuntos enviados desde el móvil, el patrón más parecido a subir audio.
  - `tools.rs`.
- **App del móvil:** `apps/android` (`dev.thingmaker.remote`, minSdk 30). Las pantallas son web, empaquetadas en la APK y servidas en un `WebView`. El código nativo se expone con `addJavascriptInterface(NativeBridge(), "ThingMakerNative")` en `MainActivity.kt`.
- **No hay ninguna función de voz hoy:** no aparecen `dictat`, `whisper`, `microphone`, `RECORD_AUDIO` ni `getUserMedia`.

## Contexto 2: lo que el usuario ya tiene funcionando (`D:\tts-stt`)

Sirve de referencia de calidad y de pieza reutilizable. Lee `README.md`, `docs/ARCHITECTURE.md` y `docs/AGENT_API.md`.

**Servidor de voz propio** (Python/FastAPI, en su PC con RTX 5090; LAN `http://192.168.1.134:8765`, público `https://dictate.jacksparrot.com`, token Bearer):
- `POST /transcribe`: WAV 16 kHz mono.
- `WS /transcribe/stream`: PCM en vivo, eventos `partial`, `final` y `done`.
- Usa Whisper large-v3 y después limpieza con un LLM. Unos 0,9 s por 20 s de audio en caliente y 2-3 s de punta a punta con limpieza.
- El protocolo está en `docs/AGENT_API.md`.
- El prompt de limpieza está en `cleanup_providers.py`: quitar muletillas, colapsar arranques en falso, nunca traducir.

**Dictado en el propio dispositivo**, ya probado (app Android "Dictate", `android/app/src/main/java/com/jacksparrot/dictate/LocalStt.kt`):
- NVIDIA **Parakeet-TDT-0.6B-v3**: modelo de 615 MB, int8, licencia CC-BY-4.0. Transcribe en streaming mientras se habla, puntúa y detecta español/inglés solo. Error de transcripción en español del 3,45 % (FLEURS).
- En un móvil va bien en CPU. **En un ordenador debería ir sobrado**; es el candidato natural para el STT local del escritorio.
- Allí corre con MediaPipe `tasks-audio` (Android). En escritorio habría que usar otro runtime: sherpa-onnx tiene un Parakeet v3 en ONNX int8; también hay otros.

## Preguntas a responder

**1. Motor STT en el escritorio (lo central).** Compara al menos:
- **(a) Parakeet v3 en local:** sherpa-onnx (¿enlazado desde Rust o como proceso aparte?), ONNX Runtime directo, u otros runtimes.
- **(b) Whisper local:** whisper.cpp o similar, con Metal en Mac y CPU, Vulkan o CUDA en Windows.
- **(c) APIs del sistema:** Apple Speech (SFSpeechRecognizer / SpeechAnalyzer) y Windows.Media.SpeechRecognition. ¿Funcionan sin conexión y con calidad en español?
- **(d) Web Speech API en el WebView:** ¿existe en WebView2 y en WKWebView? ¿Envía el audio a la nube?
- **(e) "Servidor de voz" opcional y configurable** (URL + token), compatible con el protocolo de tts-stt, para quien lo tenga.

Para cada una: calidad en español e inglés, latencia de los resultados parciales y del final, tamaño y descarga del modelo (¿empaquetado o descargado la primera vez, como hace Local Models con Ollama?), consumo de RAM y GPU, licencia (Parakeet CC-BY-4.0 exige atribución), y mantenimiento en las dos plataformas.

**2. Captura del micrófono.**
- ¿En el WebView (`getUserMedia`) o en Rust (por ejemplo `cpal`), según ADR-003?
- Permisos: `NSMicrophoneUsageDescription` y entitlements en macOS; permisos de micrófono en WebView2 en Windows.
- Formato que necesita el motor: 16 kHz mono.

**3. Experiencia de uso.**
- Pulsar y mantener, o pulsar para empezar y para parar. Atajo de teclado global o solo con la ventana activa.
- Indicador de grabación. Texto parcial en vivo en el compositor.
- Insertar en la posición del cursor y no enviar nunca automáticamente. Idioma automático o forzado.

**4. Limpieza del texto.**
- ¿Hace falta? Parakeet ya puntúa.
- Si hace falta, ¿cómo se hace sin API keys? Opciones: el mecanismo de sugerencias de respuesta (un proveedor por suscripción), Ollama (Local Models) o reglas simples.
- Qué latencia añade cada opción. Tiene que ser opcional.

**5. Flujo del móvil sobre el puente.**
- Comandos nuevos en `dispatch.rs`: subir audio y recibir el texto.
- Cómo viaja el audio por las dos vías (relay de Cloudflare y directa): ¿un solo envío al soltar o en trozos? Límites de tamaño del relay; reutilizar el patrón de `images.rs`.
- Cómo vuelve el texto, final y a ser posible parcial, al compositor del móvil para editarlo.
- En el lado Android: grabación y permiso `RECORD_AUDIO`, y cómo pasa el audio del código nativo a la pantalla web a través del `NativeBridge`.

**6. Arquitectura y pruebas.**
- Dónde vive el motor: supervisor, host de Tauri o proceso aparte.
- Cómo se apaga cuando no se usa, para liberar memoria.
- Cómo se prueba con su sistema de tests (`pnpm typecheck && pnpm test`, `cargo test`, proveedores simulados en `packages/test-fixtures`) y qué pruebas manuales hacen falta en Mac y en Windows.

## Datos de referencia

| Qué | Cifra |
| --- | --- |
| Whisper large-v3 en RTX 5090 (servidor tts-stt) | ~0,9 s por 20 s de audio en caliente |
| Limpieza con Gemini 2.5 Flash Lite | ~0,5 s |
| Parakeet v3 en un móvil Pixel 11 Pro (CPU) | En streaming y suficientemente rápido para dictar, según el usuario; sin cifras exactas registradas |
| Modelo Parakeet v3 int8 | 615 MB (ONNX de sherpa-onnx: ~640 MB) |
