# Comparación Windows: voz humana con ventilador

Fecha: 2026-11-16. Continuación de la [comparación sintética](voice-benchmark-2026-11-16/README.md). Sin cambios de producto, servidor ni configuración de modelos. Audio humano y transcripciones completas guardados localmente en `output/voice-benchmark-human/`, fuera de Git. No se enviaron a proveedores cloud.

## Muestra y método

Una grabación real del usuario dando instrucciones, con el ventilador encendido según su descripción del entorno. Micrófono Realtek, PCM16 mono a 44,1 kHz. Duración: 44,976 s, con silencios iniciales y frase final incompleta por límite de grabación. Cero muestras saturadas y ningún overflow reportado en captura. Esto no garantiza inteligibilidad ni descarta procesamiento de audio del SO.

Se remuestreó una vez a WAV mono PCM16 a 16 kHz, sin reducción de ruido, recorte, normalización o cambios de ganancia. Todos los motores recibieron ese mismo archivo completo:

- Original SHA-256: `0152933896116fd6eb37b273f3493248448848e653cdc6f4e1d5cd57b0658023`.
- Copia de prueba SHA-256: `ecf7f8f2ee5e0fd183ed8515d8dd7b7cb34f5de597e02169409646e89d92a3be`.
- Audio: `output/voice-benchmark-human/fan-20261116-230148.16k.wav`.
- Resultados completos: `output/voice-benchmark-human/fan-20261116-230148-results/`.
- Harness: `output/voice-benchmark-human/compare_fan.py` y diagnóstico `check_whisper_vad.py`.

Mismo PC que antes: Ryzen 9 9950X3D2, unos 64 GB de RAM comercial y RTX 5090. Tres ejecuciones secuenciales por motor; no se lanzaron motores de prueba simultáneamente. Mediana caliente = pasadas 2 y 3, no p95. La actividad ajena del ordenador y cachés no se controlaron. Se comparan pipelines configurados, no solo arquitecturas de modelos: el servidor Whisper aplica VAD y los demás tienen su propio tratamiento del audio.

## Tiempos medidos

Se mide desde la entrega del archivo completo hasta el resultado, **sin incluir los 45 s de grabación**. No es una prueba de primeros parciales ni de captura en vivo.

| Motor | Mediana caliente | Pasadas calientes | Primera petición de esta tanda |
| --- | ---: | ---: | ---: |
| Parakeet v3 int8, sherpa 2.0.1, CPU 4 hilos | **1,675 s** | 1,594 / 1,756 s | 1,170 s + 1,138 s de carga |
| Whisper small F16, whisper.cpp 1.9.2, CPU 4 hilos | **3,827 s** | 3,839 / 3,814 s | 3,847 s + 0,762 s de arranque |
| Whisper large-v3, faster-whisper CUDA, VAD activo | **0,788 s** | 0,780 / 0,797 s | 3,147 s, modelo inicialmente descargado de memoria según health |
| Gemma 4 12B Q4_K_M, Ollama 0.40.5, GPU | **0,787 s** | 0,787 / 0,787 s | 4,066 s |
| Limpieza Qwen3 4B sobre Parakeet, GPU | **+0,306 s** | 0,301 / 0,311 s | 1,930 s |
| Limpieza Qwen3 4B sobre Whisper GPU, GPU | **+0,296 s** | 0,297 / 0,294 s | 0,296 s; modelo ya cargado por la limpieza anterior |

Parakeet más limpieza daría aproximadamente 1,98 s, sumando medianas de fases separadas. No es un tiempo medido de un flujo completo en UI. En esta tanda la primera petición de Ollama fue mucho más corta que los 34/47 s de la tanda anterior: no se vaciaron cachés ni se reinició el equipo. Deben conservarse ambas observaciones; no sustituir una distribución fría por una única ejecución favorable.

Gemma reportó unos 8,42 GB decimales en `size_vram` mientras estaba cargado. Es una cifra del servicio, no un pico medido. Parakeet no usó GPU. Todos los motores locales mantienen coste de servicio por petición de 0 €, excluyendo electricidad, hardware y mantenimiento; las hipótesis eléctricas del informe anterior no se han medido de nuevo.

## Diferencias de contenido

No hay transcripción humana revisada ni muestra equivalente sin ventilador. Por tanto, **no se calcula WER ni se atribuyen errores causalmente al ventilador**. Se comparan diferencias observables entre salidas:

- Parakeet conserva la frase inicial, el contenido central y el final cortado. Transcripción idéntica en sus tres pasadas.
- Whisper small conserva la frase inicial, pero simplifica una expresión intermedia y omite el final cortado. Su salida lleva saltos de línea propios del runtime.
- Whisper GPU con el pipeline habitual omite la frase inicial y el final cortado; no basta con observar que es rápido.
- Gemma conserva la frase inicial y el final cortado, incluyendo una muletilla que Parakeet no incluye. La forma escrita del nombre propio queda pendiente de referencia del usuario; coincidencia entre modelos no demuestra ortografía correcta.
- La limpieza 4B elimina una expresión explicativa y cambia términos y formas verbales. Añade puntos suspensivos al final incompleto; no completa una instrucción nueva. No debe considerarse una operación literal ni utilizarse como referencia de verdad. Tampoco recupera el contenido que Whisper ya había omitido.

## Diagnóstico controlado de Whisper VAD

Se cargaron los mismos pesos locales de large-v3 con CTranslate2, misma precisión configurada, idioma español, beam size y `condition_on_previous_text=false`. Se cambió únicamente `vad_filter` entre dos transcripciones.

- `vad_filter=true`, pausa mínima 300 ms: reproduce la omisión inicial; `duration_after_vad` = 24,000 s.
- `vad_filter=false`: vuelve a aparecer la frase inicial, con audio de 44,976 s. Sigue sin recuperar el final incompleto.

Esto vincula la pérdida inicial con la etapa VAD de ese pipeline para este clip; **no prueba que eliminar VAD sea siempre mejor**, especialmente ante silencio o ruido sin voz. Las dos duraciones del diagnóstico no son comparables en caliente: la primera invocación tuvo inicialización adicional. No se usan para ordenar motores en la tabla principal.

## Conclusión provisional

En este caso real, Parakeet sigue siendo una opción atractiva para CPU: conserva más contenido que los pipelines Whisper probados y evita ocupar GPU. Gemma merece seguir en la comparación: aquí conserva contenido y da un resultado caliente rápido en la RTX 5090, aunque requiere bastante más memoria y no se puede extrapolar a un portátil.

Whisper large-v3 no queda descartado: el diagnóstico demuestra que su configuración de detección de voz requiere ajuste. No elegirlo por su latencia de 0,79 s sin contabilizar la omisión. Mantener la limpieza opcional y reversible.

Siguiente validación: referencia humana del clip, una toma completa sin corte y más ejemplos con nombres/rutas/negaciones, además del Mac. La comparación con suscripción sigue pendiente por los bloqueos previamente documentados. No se han publicado ni añadido a Git la voz del usuario o sus transcripciones completas.

## Revalidación del PR en Windows, 19 de noviembre de 2026

El ejemplo Rust `transcribe` compilado en modo release para `x86_64-pc-windows-msvc` procesó el mismo WAV humano de 44,976 s, con el modelo instalado y verificado. Produjo 383 caracteres que coinciden exactamente con las tres salidas previas de Parakeet. Se comprobó la inferencia con audio real guardado. La captura interactiva con micrófono y la revisión del texto en la aplicación quedan fuera de esta prueba.

El instalador local NSIS de Windows x64 ocupa 18.404.508 bytes (17,55 MiB), SHA-256 `d7a93f131c9474c64b664ec95948654b1b29a6b0e76531687c1471c56f1cddf1`. La compilación optimizada terminó correctamente. `dumpbin /dependents` verificó que el ejecutable de escritorio y el ejemplo de transcripción carecen de importaciones directas de la CRT dinámica. La CI de release mide por separado el DMG universal cuando se ejecuta en macOS.
