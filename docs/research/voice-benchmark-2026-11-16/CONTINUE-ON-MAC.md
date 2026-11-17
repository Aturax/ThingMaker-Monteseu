# Retomar la comparación en el Mac

## Punto de partida

Rama: `codex/desktop-voice-plan`, creada sobre `origin/main` (release 1.2.0). No contiene implementación de dictado. La investigación se hizo en Windows sobre el checkout que se identifica en las notas; verificar las rutas y contratos contra esta base antes de implementar.

Leer primero [resultados y costes](README.md) y después [propuesta de arquitectura](../../plans/voice.md). El brief de la raíz es el encargo original: la instrucción posterior del usuario limita el alcance al **ordenador**, sin Android por ahora. No hay motor elegido definitivamente.

## Recuperar la rama

En un checkout limpio:

```sh
git fetch origin
git switch --track origin/codex/desktop-voice-plan
```

Si la rama ya existe localmente, usar `git switch codex/desktop-voice-plan`. Si el checkout tiene trabajo pendiente, crear un worktree separado en vez de cambiar su rama.

## Qué queda guardado

- `results.json`: medidas originales, versiones, configuración, transcripciones y hashes.
- `reproduction/`: los cuatro WAV sintéticos exactos, manifest, resultados por ejecución y scripts utilizados. No contiene voces privadas, tokens, modelos, binarios ni dependencias descargadas.
- Los scripts son **harnesses de la sesión Windows**, no herramientas portables: contienen rutas locales y opciones de procesos de Windows. No ejecutarlos sin adaptación en macOS. Para reproducirlos en Windows, copiar el contenido a `output/voice-benchmark/` desde la raíz del repositorio; los scripts de descarga recuperan los modelos aparte. `prepare.py` requiere el servidor personal y no es necesario para los WAV ya guardados.
- No regenerar los WAV del baseline: cambiaría la comparación. Comprobar sus SHA-256 contra `manifest.json`.

## Grabaciones en la oficina

Grabar primero un corpus humano pequeño y ampliarlo si las diferencias no son claras. Mantener un original sin procesar, transcripción de referencia revisada y copia WAV mono PCM16 a 16 kHz. Guardar nuevos audios en `output/voice-benchmark-human/`, que queda ignorado por Git; no incluir conversaciones de compañeros ni datos confidenciales.

Casos prioritarios (español, inglés y mezcla):

1. Orden corta: «Cambia solo el color del botón. No modifiques ningún otro archivo».
2. Dictado de unos 20 segundos con «no hagas commit», nombres de ficheros y números.
3. Rectificación: «Pon el límite en quince, no, mejor en cincuenta. Espera, no cambies el límite todavía».
4. Dictado espontáneo de 60 segundos con pausas, muletillas y términos técnicos.
5. Silencio y ruido ambiental sin habla; debe evitar inventar texto.
6. Mismo texto con micrófono integrado y auriculares, si ambos se usarán habitualmente.

No confundir lo que se quería decir con lo que realmente se dijo: conservar referencia literal y, si se evalúa limpieza, una versión limpia esperada por separado. Obtener autorización explícita antes de enviar grabaciones humanas a proveedores cloud. Las muestras sintéticas guardadas no contienen información sensible.

## Comparación pendiente

- Parakeet/sherpa CPU y Whisper.cpp CPU/Metal, mismos WAV humanos, versiones/modelos registrados.
- Apple SpeechAnalyzer: comprobar macOS, hardware, `supportedLocales`, assets instalados, español e inglés. Registrar frío y caliente; no asumir que macOS 26 garantiza soporte en cualquier Mac.
- Medir primer parcial y latencia después de detener, además de transcripción de archivo completo. Las medidas Windows existentes son offline, sin captura de micrófono.
- Si se usa Ollama para limpiar, medir carga inicial y varias entradas distintas: repetir el mismo texto puede favorecer cachés. La primera carga Windows de 4B tardó 34 s.
- Codex: `codex login status` no encontraba cuenta en el entorno Windows de prueba. Verificar login y soporte real de audio con suscripción antes de medir; no extraer tokens ni usar una API de pago como sustituto.
- Antigravity ACP 1.5.9 anunció `audio: false`: comprobar de nuevo si cambia la versión. Su dictado interactivo no demuestra integración headless.
- Ollama 0.40.5 + Gemma 4 12B sí transcribió audio directamente en Windows. Fue más lento que Parakeet y la primera petición tardó 47 s; repetir en Mac solo si interesa esta ruta multimodal.

Hacer al menos una pasada fría y varias calientes por clip, sin ejecutar motores simultáneamente. Registrar hardware, runtime, modelo/hash, idioma, hilos/aceleración y tiempos. No presentar p95 con dos repeticiones ni interpretar el corpus sintético limpio como precisión humana.

## Decisión pendiente

Parakeet es el candidato local mejor situado por esta tanda Windows, pero falta evidencia con la voz del usuario. Mantener el envío explícito y la limpieza opcional en el diseño. El siguiente paso es medir en la oficina y actualizar el informe; no implementar todavía por inercia.
