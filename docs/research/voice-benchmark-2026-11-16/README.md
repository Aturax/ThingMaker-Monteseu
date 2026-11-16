# Comparación de dictado: primeras medidas y costes

Fecha: 2026-11-16. Investigación de escritorio; no cambia código de ThingMaker.

Continuación: [guía para el Mac y grabaciones en la oficina](CONTINUE-ON-MAC.md). Las muestras y harnesses originales se conservan en [reproduction/](reproduction/); no dependen de conservar la carpeta ignorada `output/` del PC.

## Resultado y alcance

Se ha ejecutado una primera comparación **con los mismos cuatro WAV**, no cuatro grabaciones diferentes. Parakeet CPU, Whisper.cpp small CPU, faster-whisper large-v3 GPU y Gemma 4 12B en Ollama recibieron los mismos bytes. También se midió la limpieza de las transcripciones con Ollama: 60 peticiones medidas en total, contando esta etapa adicional de texto.

Son muestras **sintéticas de Kokoro**, no grabaciones humanas. Permiten medir latencia y comprobar contratos; no bastan para elegir por precisión, acento, ruido o habla espontánea. No se ha probado micrófono ni streaming en tiempo real. No hay p95 fiable: solo tres pasadas por clip, y dos de ellas se usan para la mediana caliente. No se ha probado un Mac.

La tanda usa español e inglés, restricciones negativas, cifras y una rectificación seguida de retirada de la orden. El texto exacto, duración y SHA-256 figuran en [results.json](results.json). Los WAV y scripts están en `output/voice-benchmark/`, ignorado por Git. No se reutilizaron las grabaciones privadas encontradas en `D:/tts-stt/recordings`.

## Equipo y configuración

- Windows 11 build 26300, AMD Ryzen 9 9950X3D2, 16 núcleos/32 procesadores lógicos, 66.188.959.744 bytes de RAM física (~61,64 GiB).
- RTX 5090 identificada por el log de Ollama (31,8 GiB). La consulta NVML con `nvidia-smi` falló incluso fuera del sandbox; se conserva esa limitación de inventario.
- Parakeet TDT 0.6B v3 int8, sherpa-onnx 2.0.1, **CPU, 4 hilos**. Descarga comprimida 487.211.443 bytes, SHA-256 `1495e91b59d7fc53ddca91f3431758ec99f8fa253fdb0e1e851be0f72b14b0f9`.
- Whisper.cpp **v1.9.2**, small multilingüe F16, **CPU, 4 hilos**, beam/best-of 1. Se eligió una release con binario Windows publicado; no se afirma que sea la última versión. Pesos 487.601.967 bytes, hash y URLs en results.json.
- Servidor existente: faster-whisper 1.2.1 / CTranslate2 4.7.1, large-v3 CUDA, `cleanup=false`, idioma indicado por clip. Es una referencia de GPU potente; no el backend embebido propuesto.
- Ollama 0.40.5, `qwen3:4b-instruct` Q4_K_M, contexto 2048, temperatura 0, razonamiento desactivado, sin herramientas. Modelo enteramente en GPU según runtime; `/api/ps` reportó 2.876.160.081 bytes de VRAM, no un pico medido.
- Audio directo a Ollama: `gemma4:12b` ya instalado, con capacidad `audio` anunciada y doce respuestas HTTP 200 verificadas en `/v1/audio/transcriptions`; temperatura 0 e idioma de cada clip. Esto comprueba esa versión y modelo concretos, no que cualquier modelo de Ollama acepte audio ni que sea Whisper.

Las inferencias comparadas se ejecutaron secuencialmente entre motores. No se controlaron todos los procesos externos de la estación ni se midió potencia eléctrica. El resultado no representa un portátil corriente.

## Latencia medida

Tiempo desde que se entrega el **archivo completo** hasta que se recibe el texto. No incluye hablar, preparar WAV o descargar modelos. Para un clip de 18 s, un resultado de 0,42 s significa recibir texto aproximadamente 18,42 s después de empezar a grabar si esperamos al final para transcribir.

| Grabación | Duración | Parakeet CPU | Whisper small CPU | Whisper large-v3 GPU | Limpieza Ollama, adicional |
| --- | ---: | ---: | ---: | ---: | ---: |
| Español, orden breve | 3,78 s | 0,098 s | 1,672 s | 0,210 s | 0,091 s |
| Español, propuesta y restricciones | 12,74 s | 0,294 s | 2,030 s | 0,563 s | 0,175 s |
| Inglés, restricciones | 15,10 s | 0,353 s | 1,920 s | 0,478 s | 0,163 s |
| Español, corrección y retirada | 18,01 s | 0,418 s | 2,219 s | 0,793 s | 0,278 s |

**Audio directamente a un modelo en Ollama**, misma métrica y mismos WAV:

| Grabación | Gemma 4 12B, caliente |
| --- | ---: |
| Español, orden breve | 1,725 s |
| Español, propuesta y restricciones | 2,097 s |
| Inglés, restricciones | 2,161 s |
| Español, corrección y retirada | 3,062 s |

Se solicitó transcripción mediante el endpoint específico. No se ha medido una instrucción adicional de limpieza en una sola llamada multimodal; conservar las rectificaciones en estos resultados no prueba que el modelo las haya depurado.

Valores: mediana de pasadas 2 y 3, con modelo residente. Ollama recibió el texto literal de Whisper GPU; esa columna no es reconocimiento de audio. Los mismos textos se repitieron: puede favorecer cachés, especialmente las del LLM. Hay que añadir entradas nuevas en la validación humana.

**Arranque en frío de modelo, una observación, no una distribución:**

| Motor | Observación |
| --- | --- |
| Parakeet | Construir/cargar recognizer: 1,30 s; primera transcripción breve: 0,114 s. Suma aproximada 1,42 s. |
| Whisper.cpp | Arrancar servidor y esperar disponibilidad: 0,61 s; primer archivo: 1,68 s. Suma aproximada 2,30 s. |
| Whisper GPU existente | Primera petición: 4,48 s. `/health` confirmaba `whisper_loaded=false` antes. |
| Ollama, limpieza 4B | Primera petición: 34,17 s; posteriores: 0,09–0,30 s. Incluye carga/preparación del modelo. |
| Ollama, Gemma 4 12B audio | Primera petición: 47,19 s; posteriores: 1,72–3,28 s. El rango incluye la primera pasada de los clips largos. |

«Frío» aquí no significa reiniciar Windows ni vaciar la caché del sistema de archivos. Los motores locales se probaron con pesos ya descargados. Las descargas, actualizaciones y la carga inicial se deben mostrar como estado separado en la UI.

Para Whisper small + limpieza, sumar las dos fases medidas daría aproximadamente **1,76–2,50 s en caliente** en este equipo. Es una proyección aritmética, no una medida conjunta: la limpieza se midió sobre las salidas de Whisper GPU y no se ha demostrado que corrija los errores de Whisper small. Con el LLM descargado, la primera limpieza puede dominar toda la espera.

## Fidelidad en este corpus pequeño

Se normalizan minúsculas y puntuación, y las palabras «quince/cincuenta» se equiparan a «15/50». No se reparan palabras partidas por el runtime. Referencias: 137 palabras entre los cuatro clips.

- Parakeet y Whisper GPU: 0 errores de palabra frente al guion sintético tras esa normalización. **No equivale a precisión del 100 % con voz humana.**
- Gemma 4 12B también obtuvo 0 errores normalizados en estos cuatro clips. No ofrece aquí una ventaja de fidelidad frente a los dos anteriores; no generalizar fuera de esta muestra.
- Whisper small: 6 errores/137 palabras (4,38 % en esta muestra), incluidos «modifique» por «modifiques», «Previsa» por «Revisa» y fragmentación interna de palabras en su respuesta. Debe investigarse la fragmentación del runtime antes de atribuir todos los errores al modelo acústico.
- La limpieza 4B conservó las transcripciones de esta tanda y sus restricciones. No demuestra beneficio: el texto de entrada ya era correcto y apenas necesitaba limpieza. Hay que incluir muletillas y arranques falsos humanos antes de justificarla por defecto.

## Suscripciones y sistemas nativos: estado real

| Opción | Prueba o impedimento comprobado | Qué falta |
| --- | --- | --- |
| Codex, audio por suscripción | App-server arrancó y enumeró modelos; `account/read` no devolvió cuenta y `codex login status` indicó `Not logged in`. No se envió audio ni se usó API key. | Login en el CLI/entorno de prueba, luego mismo corpus y verificación de soporte del modelo. El schema local no prueba acceso. |
| Antigravity ACP | Handshake real con versión 1.5.9: `promptCapabilities.audio=false`. Resultado guardado. | Una interfaz oficial de audio compatible. No usar su TUI como una API. |
| Claude Code dictado | La función propia está documentada; no se encontró contrato headless de transcripción reutilizable en la revisión. | Interfaz de integración, no solo que funcione el micrófono en Claude. |
| Apple SpeechAnalyzer | No se dispone de un Mac para ejecutar esta tanda. | Mismos WAV en Mac, assets/idioma y versión del SO. |
| Windows nativo | Inventario adicional de SAPI/System.Speech: solo `MS-1033-80-DESK`, `en-US`. | No hay benchmark español. Ese inventario no determina lo disponible en Windows.Media.SpeechRecognition o Windows AI; son interfaces distintas. |
| Web Speech | No se usó como transcriptor portable ni se automatizó una interfaz de navegador. | Compatibilidad del WebView, servicio y privacidad; sigue fuera de la opción recomendada. |

## Estimaciones para lo que aún no se puede medir

Estas bandas son **hipótesis de planificación de baja confianza**, no resultados, SLAs ni cifras publicadas por los proveedores. Se refieren a clips de unos 20 s, red normal y modelo caliente. No se usan para declarar un ganador.

| Opción | Espera estimada después de terminar de hablar | Motivo de incertidumbre |
| --- | --- | --- |
| Audio completo a modelo por suscripción | Presupuestar 2–15 s; puede excederlo o rechazar la petición | Arranque del proceso, subida, cola, modelo y cuenta. No hay medición autenticada. |
| Transcripción nativa realmente incremental | Presupuestar 0,5–3 s de cierre; primer parcial debe medirse aparte | Endpointing, SO, dispositivo e idioma. Preparar assets puede añadir mucho más. |
| Parakeet con previews/VAD en este PC | Aproximadamente 0,9–1,5 s de cierre si se espera una pausa de 0,5–0,8 s más una decodificación corta | Proyección sobre medidas offline; streaming no implementado ni medido. Parar explícitamente elimina la espera de pausa. |
| Whisper small con previews/VAD | Presupuestar 2–4 s de cierre en este PC | Ventanas, cola y decodificación; no extrapolar linealmente por duración. |
| Limpieza con proveedor por suscripción | Presupuestar 1–10 s adicionales | Es una petición de texto nueva, no el dictado gratuito de la interfaz del proveedor. |

No hay una cifra defendible de arranque frío cloud o nativo sin esa prueba; registrar por separado descarga de assets, carga y primer uso. La medición local de 34 s en Ollama ilustra por qué no conviene ocultar esta distinción.

## Coste: servicio, cuota y recursos

Escenario de comparación: **1.000 dictados de 20 s = 333,33 minutos de audio**. No se compraron créditos ni activaron APIs de pago en esta investigación.

| Opción | Coste marginal de servicio por 1.000 dictados | Otros costes y condiciones |
| --- | --- | --- |
| Parakeet local | 0 € en llamadas a un proveedor | Descarga de pesos, CPU/RAM, electricidad y mantenimiento. Atribución CC-BY-4.0. |
| Whisper local | 0 € en llamadas a un proveedor | CPU/GPU y modelo según variante. El resultado small CPU no requiere la RTX. |
| Whisper + Ollama local | 0 € en llamadas a un proveedor | Dos etapas, VRAM/RAM adicional y arranque del LLM. No es gratis en recursos. |
| Audio a Ollama local | 0 € en llamadas a un proveedor con modelo local | Modelo multimodal potencialmente mayor; comprobar que no usa una etiqueta cloud. |
| Codex con suscripción | **No cuantificable por minuto con la evidencia actual** | Puede estar incluido en cuota del plan o consumir créditos según cuenta. No equivaler «tengo suscripción» a uso ilimitado ni usar precios API para estimarlo. |
| Antigravity con suscripción | **No cuantificable por minuto**; integración de audio ACP no disponible aquí | Cuotas y créditos del plan. No habilitar fallback a créditos para la prueba. |
| Claude Code, dictado propio | Documenta que no consume mensajes/tokens ni cuota de `/usage` | No demuestra que podamos consumir ese servicio desde ThingMaker. Una limpieza con Claude sí sería otra inferencia. |
| APIs locales del SO | Sin tarifa por petición de proveedor prevista para procesamiento local | SO/hardware y assets; no confundir con Azure/Apple cloud. Windows AI sigue experimental según documentación revisada. |
| Servidor personal de Whisper | 0 € de tarifa STT externa si el servidor es propio y `cleanup=false` | Hardware, energía, disponibilidad y red; limpieza API configurada en el servidor se excluyó. |

La [documentación de precios de Codex](https://learn.chatgpt.com/docs/pricing) distingue consumo de suscripción de precios API. El precio mensual publicado de Plus ($20) no permite calcular un precio por minuto de dictado. La [documentación de créditos de Antigravity](https://antigravity.google/docs/cli/credits/) explica el consumo adicional de créditos si se habilita. La condición del dictado de Claude procede de su [documentación oficial](https://code.claude.com/docs/en/voice-dictation).

**Ejemplo eléctrico, no medida:** a 0,25 €/kWh, mantener una máquina consumiendo 100 W durante las 5,56 horas de captura suma unos 0,14 €; a 350 W, unos 0,49 €. Es consumo total supuesto durante ese tiempo, no consumo incremental del STT. No atribuirlo íntegramente al dictado si el ordenador ya estaba trabajando. Para estimar solo la inferencia: `vatios_adicionales / 1000 × segundos_de_cómputo / 3600 × €/kWh`. Falta medir vatios reales; tampoco se amortizó hardware.

El coste de ingeniería sigue siendo mayor que el consumo por llamada: aproximadamente 8–15 jornadas para una integración sobre servicio compatible o 10–18 para runtime/modelos empaquetados, según la propuesta. Convertirlo a euros requiere la tarifa del equipo. Esta tanda no valida esos calendarios ni convierte un bloqueo de proveedor en trabajo ya resuelto.

## Recomendación tras esta tanda

Parakeet CPU merece pasar a la prueba con voz humana: tiene buena latencia en estas muestras sin ocupar GPU. Whisper small queda como comparador y fallback, no se descarta por este corpus mínimo. Gemma 4 en Ollama confirma que audio directo local es viable, pero no gana en latencia ni en fidelidad en esta tanda y su arranque frío es considerable. La limpieza debe permanecer opcional: el arranque frío puede costar mucho y todavía no se ha demostrado beneficio sobre dictado real.

Mantener abierta la vía con suscripción hasta probar Codex autenticado. Antigravity ACP no debe figurar como backend listo para recibir audio en su versión instalada. La comparación nativa de Mac y la calidad con la voz del usuario quedan pendientes; no hay selección definitiva de motor.

## Reproducibilidad y límites

Scripts y muestras locales: `output/voice-benchmark/prepare.py`, `bench_local.py`, `bench_parakeet.py`, `bench_whisper_cpp.py`, `bench_ollama.py`, `bench_ollama_audio.py`, `bench_codex.py`, `probe_agy.py`, `summarize.py`. Los scripts de medición usan Python 3.12 del entorno existente; no modifican ese entorno. sherpa se instaló aisladamente en `output/voice-benchmark/deps`.

Los servidores temporales de Whisper.cpp y Ollama se cerraron al terminar mediante `finally`. No se detuvo el servidor personal preexistente. No se cambió la configuración de proveedores, no se inició un agente con herramientas y no se hicieron commits. Las dependencias y modelos descargados permanecen bajo `output/voice-benchmark` para repetir la prueba.

Próxima tanda necesaria: grabaciones humanas autorizadas, micrófono real, clips de 20 y 60 s, ruido y mezcla de idiomas; entradas distintas entre repeticiones; controles de silencio; medición de primer parcial, cancelación y revisión del borrador. Sin eso, no trasladar estas cifras a una promesa de experiencia de uso.
