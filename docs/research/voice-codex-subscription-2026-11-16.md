# Prueba de audio en Codex con el perfil de ThingMaker

Fecha: 2026-11-16. Resultado: **la ruta de audio directo probada no sirve como transcriptor con el modelo de sugerencias configurado**. Login y peticiones de texto sí funcionan. Esto no demuestra que todas las interfaces de voz de ChatGPT carezcan de soporte; se refiere al programa, cuenta, modelo y contrato siguientes.

## Perfil verificado

- Configuración leída de la base de datos de ThingMaker en modo solo lectura: proveedor de sugerencias `codex`, modelo `gpt-6-luna`.
- Sin cuenta secundaria seleccionada ni ejecutable alternativo configurado: se usa la cuenta predeterminada y el binario resuelto en PATH, como indica `state.rs`.
- Binario: `codex-cli 0.167.0-alpha.4.1`, instalación de Codex de este usuario.
- `codex login status`, ejecutado bajo el usuario propietario: **Logged in using ChatGPT**. `account/read` de app-server devuelve tipo **chatgpt**. No se leyeron ni copiaron tokens.
- Entorno construido con la lista de variables heredadas de `EnvironmentProfile::trusted_local()`: no se heredan variables OPENAI/CODEX del asistente. Para la cuenta predeterminada no se inyecta un `CODEX_HOME` secundario.
- Configuración de prueba por proceso: shell, MCP, aplicaciones, hooks y otras herramientas deshabilitadas; threads efímeros, solo lectura, sin contexto del proyecto. No se cambiaron ajustes persistentes.

La prueba anterior que decía `Not logged in` se ejecutaba bajo el usuario del sandbox. **Ese resultado no representaba la autenticación del usuario de ThingMaker**. La repetición corrige esta limitación; no requiere volver a iniciar sesión en la app.

## Pruebas

Se reutilizaron los cuatro WAV sintéticos de la comparación Windows, verificando su SHA-256. No se envió la grabación humana con ventilador.

1. `turn/start` con texto de instrucción y `{type: "localAudio", path: ...}`: cuatro clips en español/inglés. Se hizo una primera pasada y una confirmación que recoge tanto deltas como mensajes finales de agente.
2. Control de texto sin audio: «Return exactly CONTROL_OK».
3. El clip corto se repitió como `{type: "audio", url: "data:audio/wav;base64,..."}`, para comprobar la alternativa inline sin depender de la ruta local.

| Caso, pasada de confirmación | Tiempo hasta terminar | Resultado |
| --- | ---: | --- |
| Español corto, `localAudio` | 2,30 s | `[audio no disponible]` |
| Español medio, `localAudio` | 2,10 s | Sin texto de respuesta |
| Inglés medio, `localAudio` | 2,01 s | `I can’t access the audio content to transcribe it.` |
| Español con corrección, `localAudio` | 2,33 s | Frase ajena al audio: «El paquete debe incluir los documentos originales, no las copias.» |
| Control solo texto | 2,11 s | `CONTROL_OK`, correcto |
| Español corto, audio inline | 2,01 s | Sin texto de respuesta |

La frase esperada del clip corto era «Cambia solo el color del botón. No modifiques ningún otro archivo». El clip de corrección trataba de cambiar un límite y luego retirar esa instrucción; no mencionaba un paquete de documentos. No se interpreta la frase inventada como una transcripción parcial.

Todas estas peticiones terminaron con estado de turno `completed`, sin error RPC. **Un turno completado no implica que el modelo haya procesado el audio.** Los tiempos de esta tabla son tiempos de respuesta o fallo funcional, no latencias de transcripción exitosa. No procede calcular WER ni compararlos como si ganasen a un transcriptor local.

El catálogo `model/list` de esta sesión anuncia texto e imagen, no audio, para los modelos enumerados. El hecho de que el schema del cliente admita `localAudio` no acredita soporte efectivo en el modelo/backend contratado.

## Consecuencia para ThingMaker

No implementar «audio → gpt-6-luna → transcripción» sobre esta vía como si estuviera validada. El patrón de sugerencias **sí permite una petición auxiliar de texto**, por lo que queda disponible la arquitectura «STT local → limpieza con la suscripción», pendiente de medir su calidad y latencia específicas.

No se probó aquí una sesión realtime, otro modelo fuera del catálogo, una API de transcripción con clave o un endpoint privado. Esas rutas no se pueden dar por verificadas ni por descartadas a partir de esta prueba.

## Evidencia local

- Harness: `output/voice-benchmark/bench_codex_thingmaker.py`.
- Perfil, catálogo y respuestas completas: `output/voice-benchmark/codex-thingmaker-profile/confirmation/`.
- Control positivo: `output/voice-benchmark/codex-thingmaker-profile/control/`.
- Entrada inline: `output/voice-benchmark/codex-thingmaker-profile/inline/`.
- La carpeta raíz de esa evidencia conserva diagnósticos de dos arranques fallidos del harness por una clave de override MCP mal construida. Se corrigió antes de las pruebas autenticadas; no es un fallo de audio del proveedor.

No se hicieron cambios de producto, commits o publicaciones. La documentación general del contrato está en [Codex app-server](https://learn.chatgpt.com/docs/app-server); los resultados concretos anteriores proceden de las llamadas realizadas, no de inferencias sobre esa documentación.
