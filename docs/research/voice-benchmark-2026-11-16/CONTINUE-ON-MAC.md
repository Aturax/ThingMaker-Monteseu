# Retomar el dictado en el Mac

## Estado actual

La rama `codex/desktop-voice-plan` **ya incluye una implementación de producto**: Parakeet TDT v3 int8, sherpa-onnx CPU, captura nativa CPAL y limpieza de texto opcional. El commit `062eceb` añadió la primera versión después de la investigación Windows. No es necesario volver a elegir motor antes de probar esta versión.

Leer [uso y arquitectura actuales](../../voice.md), [validación macOS](../voice-validation-mac-2026-11-17.md) y, para contexto histórico, [resultados Windows](README.md) y [propuesta original](../../plans/voice.md). El alcance sigue limitado al ordenador; Android/Remote no expone dictado.

## Generar y probar

Desde un checkout limpio de esta rama:

```sh
pnpm install --frozen-lockfile
pnpm --filter @thingmaker/desktop exec tauri build --bundles app --config src-tauri/tauri.dev.conf.json --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
```

La app queda en `target/release/bundle/macos/ThingMaker Dev.app`, usa el identificador `dev.thingmaker.desktop.dev` y datos separados de la instalación habitual. El paquete de distribución conserva el objetivo universal Intel + Apple Silicon en CI. Una compilación ARM local no demuestra soporte Intel.

1. Abrir la app, ir a Ajustes → Chat y descargar el modelo. Comprobar cancelación, reintento y navegación durante descarga/extracción.
2. Comprobar micrófonos y su actualización tras conectar/desconectar dispositivos.
3. Pulsar Dictar. macOS pide permiso explícito antes de abrir la captura. Probar permiso concedido y denegado; si el diálogo hace perder foco, volver a la app y pulsar Dictar otra vez. Un permiso tardío no debe iniciar una captura que ya se canceló.
4. Probar detener, Esc, cambio de ventana/pantalla, límite de 120 segundos y revisión/inserción/deshacer. El envío sigue siendo explícito.
5. Repetir con limpieza opcional y fallos del proveedor. Conservar el original y no sustituir ediciones con resultados tardíos.

## Corpus y límites de la evidencia

Los WAV sintéticos originales y sus SHA-256 están en `reproduction/manifest.json`. No regenerarlos. Los scripts originales de esa carpeta contienen rutas/opciones de Windows; no ejecutarlos sin adaptación. Los ejemplos Rust `prepare`, `transcribe` y `devices` permiten verificar descarga, inferencia y enumeración nativa sin usar esos scripts.

Para voz humana: conservar un original sin procesar, una referencia literal revisada y, si se evalúa limpieza, otra referencia limpia esperada. Guardar audios en `output/voice-benchmark-human/`, ignorado por Git. No incluir voces de compañeros ni información confidencial. No enviar grabaciones humanas a proveedores cloud sin autorización explícita.

Casos prioritarios: órdenes cortas; español, inglés y mezcla; negaciones como «no hagas commit»; rutas y nombres; números y rectificaciones; 60 segundos espontáneos; silencio/ruido sin habla; micrófono integrado y auriculares.

Medir carga del modelo y tiempo desde detener hasta el resultado. Esta versión no tiene parciales en vivo y libera el modelo después de cada dictado. No presentar p95 con pocas repeticiones ni extrapolar calidad sintética a voz humana. Whisper/Metal, Apple SpeechAnalyzer y las vías con suscripción siguen siendo investigaciones opcionales, no requisitos de esta primera implementación.
