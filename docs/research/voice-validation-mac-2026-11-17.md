# Validación de dictado en macOS — 2026-11-17

**Actualización:** el fallo puntual de la primera tanda se investigó y corrigió posteriormente; véase [causa, reproducción antes/después y validación](voice-capture-overrun-mac-2026-11-17.md). Los resultados siguientes conservan el historial de la primera validación.

## Entorno y alcance

MacBook Pro M4 Pro (12 cores, 48 GB), macOS 26.6.2, ARM64. Rama `codex/desktop-voice-plan`, base `062eceb` más las correcciones de esta sesión. Se usó `ThingMaker Dev.app`, con identificador y carpeta de datos separados de la instalación habitual. No se publicaron cambios ni se sobrescribió la app habitual.

## Problemas reproducidos y correcciones

- **Instalación del modelo entre pantallas:** salir de Ajustes durante descarga/extracción y regresar borraba el progreso y el botón Cancelar. Un nuevo intento solo mostraba «A voice model download is already running». El estado ahora pertenece a la app y sobrevive al desmontaje de la pantalla. Una prueba de regresión cubre salir/volver, conservar progreso, cancelar y finalizar.
- **Primer acceso al micrófono:** CoreAudio quedó bloqueado al crear el stream antes de completar el consentimiento, reteniendo la operación nativa tras cancelar la UI. Se añadió una solicitud asíncrona explícita mediante AVFoundation antes de abrir el dispositivo. `voice_start` comprueba autorización y foco de ventana; el frontend comprueba de nuevo la identidad de la operación después del permiso, para que un permiso tardío no abra una captura cancelada. La denegación muestra instrucciones de recuperación. La solicitud llegó a TCC (registro `AUTHREQ_PROMPTING`, `kTCCServiceMicrophone`). El usuario concedió el permiso en el diálogo del sistema para la app de prueba. Después se verificaron captura real, revocación desde Ajustes del Sistema, error de permiso denegado y restauración del acceso (con el reinicio de la app solicitado por macOS). El permiso de la app de prueba quedó restaurado. La app optimizada final solicitó consentimiento de nuevo al cambiar el ejecutable firmado; el usuario lo concedió también y se completó la comprobación acústica en ese paquete final.
- **Lista de micrófonos sin diagnóstico/actualización:** los errores de enumeración ya no se silencian y se añadió Actualizar micrófonos. Las consultas CoreAudio se ejecutan fuera del hilo de interfaz. El ejemplo nativo `cargo run -p thingmaker-voice --example devices` detectó el micrófono integrado. Se comprobó visualmente la opción «MacBook Pro Microphone» y su selección. El árbol de accesibilidad omite algunas opciones no seleccionadas de este menú; no debe usarse por sí solo para concluir que faltan dispositivos.
- **Documentación antigua:** el documento de continuación en Mac y la cabecera del plan ya distinguen la implementación actual de la investigación anterior.

## Comprobaciones realizadas

- App de prueba debug empaquetada: abre, muestra dictado y redirige a Ajustes cuando falta el modelo.
- Paquete optimizado firmado ad hoc: abre y muestra el modelo instalado. `codesign --verify --deep --strict` pasa; incluye `com.apple.security.device.audio-input` y `NSMicrophoneUsageDescription`. No se ha notarizado ni probado como universal.
- En la app firmada se pudo cancelar la espera del permiso y seguir navegando por Ajustes sin bloqueo de la interfaz. La revisión más reciente muestra «Waiting for microphone permission…» antes de abrir CoreAudio.
- Captura real con el micrófono integrado: medidor activo, parada y transcripción de un WAV sintético reproducido por los altavoces. La toma contenía además habla de fondo de la oficina: se descartó durante la revisión, sin guardar audio ni transcripciones de esa toma y sin activar limpieza cloud. Esto demuestra el flujo acústico, no precisión con voz humana aislada.
- Revisión/edición del resultado en UI, cambio del borrador antes de insertar, inserción que conserva ese cambio y deshacer que restaura exactamente el borrador anterior. No se pulsó Enviar ni se inició una sesión por el dictado.
- Cmd+Shift+D inicia y Esc cancela en la app; no inserta resultados tras cancelar. Se observó también captura activa con «MacBook Pro Microphone» seleccionado explícitamente.
- `cargo clippy -p thingmaker-voice -p thingmaker-desktop --lib -- -D warnings`: correcto tras corregir avisos de estilo del módulo de voz. La comprobación con `--all-targets` encuentra además un aviso previo ajeno al dictado en `commands/screen_tools.rs:606` (`cloned_ref_to_slice_refs`); no se modificó ese test.
- Descarga real desde la UI: cancelar y reintentar; instalación completa con comprobación SHA-256 y extracción. La inferencia posterior usa esos archivos verificados. Modelo instalado: aproximadamente 643 MiB.
- `pnpm typecheck`: correcto.
- `pnpm test`: 629 pruebas desktop y 2 de contratos, sin fallos.
- `cargo test --workspace`: 502 pruebas pasan, 8 omitidas, sin fallos. Las omitidas no se cuentan como validación.
- Pruebas nuevas: navegación durante descarga, actualización/error de dispositivos, permiso denegado, permiso tardío tras cancelar, atajo Cmd+Shift+D, Esc, pérdida de foco y resultados nativos tardíos.
- La lista de comandos Remote continúa excluyendo todos los `voice_*`; se añadió una aserción explícita a la prueba de esa frontera.

## Comprobación final del paquete optimizado

Tras el segundo consentimiento del usuario, se probó el ejecutable de `target/release/bundle/macos/ThingMaker Dev.app`, sin recompilarlo después:

- Captura real con el micrófono integrado seleccionado; medidor y contador activos.
- Reproducción acústica del WAV español corto. Al detener, la transcripción fue exactamente «Cambia solo el color del botón. No modifiques ningún otro archivo.».
- Insertar el texto y deshacer en la UI funcionaron; el borrador volvió a estar vacío. No se pulsó Enviar.
- Otro ciclo de inicio alcanzó grabación y se canceló con Esc. La app quedó inactiva y lista para volver a dictar.
- El primer intento inmediatamente después de conceder permiso mostró «Microphone disconnected or audio capture overflowed. Please try again.». Los dos intentos siguientes funcionaron (transcripción completa y cancelación). La investigación posterior identificó un Xrun de CoreAudio tratado erróneamente como fatal. La [corrección y su reproducción controlada](voice-capture-overrun-mac-2026-11-17.md) sustituyen la evaluación provisional de esta primera tanda.

App ARM optimizada de aproximadamente 56 MiB, firma ad hoc verificada. ZIP local de aproximadamente 23 MiB: `output/ThingMaker-Voice-Test-mac-arm64.zip`. La instalación habitual permanece independiente.

## Inferencia nativa con archivos

Se verificaron los hashes del manifest original y se ejecutó el ejemplo Rust `transcribe` tres veces por archivo, en procesos separados que cargan el modelo cada vez. Las 21 ejecuciones finalizaron correctamente; cada archivo produjo el mismo resultado en sus tres pasadas.

| Entrada | Resultado |
| --- | --- |
| Español corto, 16 kHz | Conserva la restricción «no modifiques ningún otro archivo»; diferencia de puntuación respecto a referencia |
| Español medio, 16 kHz | Coincide con el texto de referencia |
| Inglés medio, 16 kHz | Coincide con el texto de referencia |
| Rectificaciones, 16 kHz | Conserva negaciones y rectificación de 15 a 50; escribe cifras y cambia puntuación |
| Español corto remuestreado a 48 kHz | Mismo resultado que 16 kHz |
| Silencio digital, 3 s | Texto vacío |
| Ruido pseudoaleatorio suave, 3 s | Texto vacío |

El remuestreo de prueba usa interpolación lineal; el ruido usa semilla 17, PCM16 uniforme entre -100 y 100. No son ruido de oficina ni grabaciones humanas. Los resultados completos están localmente en `output/voice-validation-mac/file-transcription.json` (ignorado por Git).

Las medianas observadas de proceso completo estuvieron aproximadamente entre 1,0 y 2,5 s para los clips con voz. **No son un benchmark de la app optimizada:** se usó el binario debug y había compilaciones concurrentes. No se calcula p95 ni WER, ni se extrapola a la voz del usuario.

## Límites y comprobaciones pendientes

1. Auriculares USB/Bluetooth, desconexión durante captura, cambio real de foco/minimización/cierre durante grabación, suspensión y límite real de 120 s. Los eventos de pérdida de foco y desmontaje están cubiertos con pruebas simuladas; las acciones dirigidas por app de la herramienta no demostraron un cambio real de foco del sistema. Solo estaba disponible el micrófono integrado; el test unitario verifica que el buffer no excede su límite.
2. Limpieza mediante proveedor real, incluidos timeout/error y conservación de negaciones, nombres y cifras. En este Mac no hay modelos locales de Ollama disponibles. No se envió audio humano ni transcripción de la toma de oficina a ningún proveedor. Las pruebas automatizadas cubren el fallo/resultado tardío y la restricción de sesiones locales.
3. Paquete universal y ejecución en un Mac Intel. El toolchain local solo tiene la biblioteca estándar ARM; la configuración universal de CI se conserva.
4. Calidad y latencia con voz humana aislada, español/inglés/mezcla y ruido real. No están demostradas por los WAV sintéticos ni por una única reproducción acústica en oficina.
5. Transición real de StartingPanel a SessionPanel durante un dictado y sesiones con envío/cola/steer: la lógica existente cancela al desmontarse el compositor; no se enviaron mensajes para forzar esas transiciones en esta sesión.

## Reproducir el paquete optimizado

```sh
pnpm --filter @thingmaker/desktop exec tauri build --bundles app --config src-tauri/tauri.dev.conf.json --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
```

Salida: `target/release/bundle/macos/ThingMaker Dev.app`. Firma ad hoc para prueba local; no notarizada. No es un instalador universal ni una publicación.
