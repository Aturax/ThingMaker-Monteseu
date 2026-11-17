# Fallo de captura macOS: investigación y corrección

## Problema y evidencia original

La primera captura de la app optimizada, tras conceder permiso, terminó con el mensaje ambiguo «Microphone disconnected or audio capture overflowed». Un reintento funcionó, pero eso no demostraba que el fallo estuviera resuelto.

La consulta dirigida del registro unificado de macOS recuperó el incidente original del proceso ThingMaker 75571:

- 13:02:58.935: CoreAudio marca la entrada del proceso como activa.
- **13:03:00.671:** `HALC_ProxyIOContext::IOWorkLoop: skipping cycle due to overload`.
- **13:03:00.688:** se detiene la captura del proceso, 17 ms después.
- 13:03:00.811: `HALS_OverloadMessage: Overload possibly due to HAL client proc exceeding io cycle budget`.

No hay evidencia de una desconexión física en ese intervalo. El registro identifica una sobrecarga de la ruta de audio, pero no permite atribuir por sí solo el retraso a una asignación concreta, al planificador o a otra actividad del sistema.

## Causa del aborto en ThingMaker

El callback de error descartaba el tipo de error CPAL y activaba el mismo booleano fatal para todo. El bucle de captura abortaba tanto ante una cola propia llena como ante un aviso `ErrorKind::Xrun` del host.

La implementación macOS de CPAL 0.18.2 registra `kAudioDeviceProcessorOverload` y comunica `Xrun` sin detener el stream. Su ruta de desconexión, en cambio, pausa el stream antes de reportar el fallo. Tratar ambos sucesos igual convertía un aviso recuperable en la pérdida de toda la toma.

Referencias primarias: [CPAL 0.18.2, CoreAudio](https://docs.rs/crate/cpal/0.18.2/source/src/host/coreaudio/macos/mod.rs), [categorías de error](https://docs.rs/cpal/0.18.2/cpal/enum.ErrorKind.html).

También se encontró un riesgo evitable en el callback de datos: creaba un `Vec` nuevo por bloque y usaba `std::sync::mpsc::SyncSender::try_send`. Ese canal usa sincronización interna; `try_send` no lo convierte en una estructura apta para tiempo real. Se retiraron estas operaciones del callback. No se afirma que fueran la causa exclusiva del incidente original.

## Cambios

- `Xrun` incrementa un contador y mantiene el stream. El worker emite una advertencia explícita de posible interrupción del audio, una sola vez por toma.
- Esa advertencia permanece visible durante la grabación y la revisión del resultado, incluso si falla la limpieza opcional. Ni la advertencia ni la transcripción insertan/envían texto automáticamente. No se pretende reconstruir audio que el host no entregó: el usuario debe revisar o repetir la toma.
- Overflow de la cola propia, desconexión, permiso revocado y otros fallos del stream siguen siendo fatales. Los mensajes distinguen estas situaciones. La primera causa fatal se conserva.
- El callback de datos escribe muestras mono en un anillo SPSC `rtrb 0.4.1`, preasignado y sin bloqueos. No asigna/libera memoria, no usa canales con mutex y no despierta otros hilos. La capacidad es de un segundo a la frecuencia real del dispositivo, independiente del tamaño de bloque.
- El búfer de grabación acotado y las páginas del anillo se preparan antes de arrancar el stream. El worker drena el anillo cada 10 ms. Si no puede consumir a tiempo, el productor falla sin sobrescribir audio pendiente y la toma se aborta; no se oculta una pérdida de muestras en la cola propia.
- Se registran únicamente frecuencia, canales, número de callbacks/muestras, duración máxima del callback y códigos/contadores de fallo. No se registran audio ni transcripciones.
- Se añadió el ejemplo opt-in `capture`, que ejecuta el ciclo real de Service y nunca imprime ni guarda transcripciones. Puede alternar detener/cancelar y comprobar una frase esperada sin mostrar el contenido reconocido.

## Reproducción controlada antes/después

Se conservó localmente el ejecutable del probe con la ruta anterior, instrumentada para distinguir la causa pero manteniendo el aborto ante `Xrun`. Se comparó con el probe corregido, mismo Mac y micrófono integrado a 48 kHz.

En cada prueba se inició una toma de 5 s y, a los 2 s del proceso, se suspendió **solo ese proceso de prueba** mediante `SIGSTOP` durante 250 ms, seguido de `SIGCONT`. Esto induce un incumplimiento real del plazo de CoreAudio sin modificar el driver ni la app habitual.

| Caso | Resultado |
| --- | --- |
| Antes | `failure=2` (Xrun), `failures=1`, salida 1: aborta la toma |
| Después | `failure=0`, `host_overruns=1`, `warnings=1`, salida 0: continúa y completa la transcripción |

Se repitió además con el probe corregido, toma de 8 s, la misma pausa de 250 ms y reproducción del WAV sintético `es_short.wav` inmediatamente después de reanudar. El resultado conservó «Cambia solo el color del botón no modifiques ningún otro archivo» (comparación normalizada sin puntuación), con **`expected_match=true`, una advertencia y cero fallos**. Esto comprueba que el audio posterior al aviso sigue llegando al transcriptor; no demuestra que se pueda recuperar el intervalo perdido.

## Validación adicional

- 20 ciclos reales con la ruta anterior instrumentada, sin interrupción inducida: 20 correctos. El fallo era intermitente; no se usa esta tanda como reproducción.
- 30 ciclos reales con la corrección, alternando 20 transcripciones y 10 cancelaciones: **30 correctos, cero fallos y cero avisos**. Hubo compilaciones concurrentes; no se presenta como benchmark ni garantía de ausencia de futuros overruns del sistema.
- Un allocator de prueba cuenta asignaciones **y liberaciones** en el hilo que ejecuta el callback real: cero, incluyendo conversión estéreo, vuelta del anillo y anillo lleno.
- Tests nativos de continuidad de muestras tras dos Xrun y una sola advertencia; overflow sin sobrescritura; desconexión fatal y conservación de la primera causa.
- Test de interfaz: la advertencia no detiene la grabación, sobrevive al resultado y a un fallo de limpieza, no modifica el borrador y se retira al descartar.
- `pnpm typecheck` correcto. Suite frontend: **630 desktop + 2 contratos**, sin fallos.
- `cargo test --workspace`: **505 pasan, 8 omitidos, cero fallos**.
- `cargo clippy -p thingmaker-voice --all-targets -- -D warnings` y `cargo clippy -p thingmaker-desktop --lib -- -D warnings`: correctos.

Los logs de los probes, sin audio ni transcripciones, están en `output/voice-validation-mac/capture-regression/`, ignorado por Git. El binario anterior queda en `output/voice-validation-mac/capture-before` solo como evidencia local.

## Repetir el probe corregido

```sh
cargo run -p thingmaker-voice --example capture -- "$HOME/Library/Application Support/dev.thingmaker.desktop.dev" 30 1 mixed
```

Para la prueba interrumpida, iniciar el mismo ejecutable con `1 8 stop` y, opcionalmente, la frase esperada como último argumento. Suspender únicamente el PID hijo creado para esa prueba durante 250 ms y reanudarlo, manteniendo un `finally` que garantice `SIGCONT` y limpieza del hijo si falla el harness. Reproducir el WAV sintético tras reanudar. No detener procesos ajenos ni guardar conversaciones del entorno.

## Paquete y límites

La app optimizada ARM se ha vuelto a generar con estos cambios y firma ad hoc. **La comprobación integrada está completada** sobre ese ejecutable, después del consentimiento de micrófono del usuario:

- Se observó grabación activa en la UI y se suspendió exclusivamente el proceso de la app de prueba durante 250 ms, garantizando su reanudación en `finally`.
- La app mantuvo la grabación y mostró la advertencia de interrupción, en lugar de abortar.
- Tras reanudar, se reprodujo el WAV español por los altavoces. Al detener, el resultado fue «Cambia solo el color del botón, no modifiques ningún otro archivo.» y la advertencia siguió visible junto al área de revisión.
- Se descartó el resultado: se retiraron la revisión y la advertencia; el borrador permaneció vacío y la app quedó inactiva, lista para dictar. No se envió ningún mensaje.
- El primer control acústico integrado devolvió «No speech was recognized» porque los altavoces estaban silenciados. Se confirmó el estado Mute en Ajustes del Sistema. Con autorización explícita del usuario se desactivó el silencio solo para repetir la reproducción y se restauró inmediatamente después; el volumen no se cambió. Esa toma inicial también mantuvo la captura tras Xrun, sin fallo fatal.
- Los diagnósticos de ambas tomas registraron `failure=0` y `host_overruns=1`. No se guardaron audio ni transcripciones de micrófono; solo se documenta la frase sintética no sensible.

El ZIP actualizado contiene exactamente el mismo ejecutable probado. SHA-256 del ejecutable: `1d5e95871699a1236178d8fe6dbae704c479a070960499a45a0576b2e3984156`. La firma de la app se verificó con `codesign --verify --deep --strict`.


La corrección elimina el aborto erróneo ante un aviso recuperable y retira trabajo no apto para tiempo real del callback. No garantiza que macOS nunca pierda un ciclo. Se conserva una advertencia visible si ocurre, y permanecen los límites documentados de calidad humana, Intel, auriculares, suspensión y desconexión física real.
