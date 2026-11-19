# Dirigir ThingMaker por voz: investigación y propuesta de escritorio

Implementación inicial autorizada y desarrollada en `codex/desktop-voice-plan`: Parakeet CPU nativo, descarga verificada, transcripción provisional aproximadamente cada 2 s con pasada final al detener y limpieza opcional con selección independiente de proveedor/modelo. Véase [uso, arquitectura implementada y validación](../voice.md). El resto de este documento conserva la investigación y las alternativas anteriores; el worker de esta primera versión es un hilo nativo con liberación del modelo al terminar, no un proceso auxiliar.

Fecha de la investigación original: 2026-11-16. **El cuerpo siguiente es histórico y no es la lista de pendientes de la implementación actual.** Las recomendaciones de no implementar/no elegir motor y las alternativas de arquitectura quedaron superadas por la implementación descrita arriba. Ya existe una [primera comparación medida y estimación de costes](../research/voice-benchmark-2026-11-16/README.md), con muestras sintéticas idénticas para los motores locales. Las afirmaciones de «no medido» del cuerpo siguiente corresponden a la investigación inicial; consultar también la [validación macOS](../research/voice-validation-mac-2026-11-17.md).

Actualización posterior: [Codex con el perfil real de ThingMaker](../research/voice-codex-subscription-2026-11-16.md) autentica correctamente mediante ChatGPT, pero `gpt-6-luna` no devuelve transcripciones válidas con `localAudio` ni con audio inline. El control de texto sí funciona. Esta evidencia sustituye el bloqueo inicial de login para esa prueba y desaconseja esa ruta como backend de dictado; no descarta otras interfaces de voz no probadas.

## 1. Objetivo y decisión propuesta

Hablar al ordenador para redactar instrucciones, iniciar sesiones y dirigir agentes que ya están trabajando. Windows y macOS; Android y el puente móvil quedan fuera por decisión del usuario. Tampoco se incluye lectura de respuestas en voz alta, escucha permanente o navegación completa por voz en la primera versión.

**No elegir todavía Parakeet por defecto.** Hay alternativas actuales que merecen una prueba comparativa: audio mediante el programa oficial de un proveedor con su sesión iniciada, transcripción local (Whisper/Parakeet/Ollama) y APIs nativas. La recomendación es una interfaz de dictado independiente del proveedor del agente, con un backend inicialmente elegido después de una prueba de viabilidad de 2–4 días. No implementar cinco motores de producción para compararlos.

Orden de evaluación recomendado:

1. **Codex app-server con audio**, porque el binario instalado expone un contrato concreto y permitiría evitar descargar un modelo. Probar archivo acotado antes que conversación de voz. Acceso con suscripción y transcripción fiel aún no demostrados.
2. **Whisper local + limpieza opcional en Ollama**, frente a **Parakeet v3/sherpa-onnx sin limpieza**: referencia independiente de cuentas, con el mismo corpus y hardware.
3. **SpeechAnalyzer en Mac**, como candidato serio para esa plataforma. Windows nativo requiere distinguir APIs antiguas, nuevas y el dictado del propio sistema.
4. **Ollama como motor de transcripción**, si una versión publicada y un modelo compatible verificable permiten simplificar el despliegue. No confundir esto con ejecutar Whisper por separado y usar Ollama solo para limpiar.

Si la vía con suscripción pasa los criterios, ofrecerla como opción explícita «Usar mi proveedor», independiente del proveedor de la sesión. Si no pasa, lanzar una primera versión local con el ganador Whisper/Parakeet. No hacer fallback silencioso de local a nube. La arquitectura admite cambiar de backend sin rehacer el compositor.

## 2. Qué se verificó y qué no

### Código actual de ThingMaker

- `apps/desktop/src/components/SessionPanel.tsx`: borradores por sesión, textarea, adjuntos, envío, cola y steering. Dictar debe alimentar el borrador; el envío conserva la lógica actual y sus capacidades.
- `apps/desktop/src/components/WorkspaceView.tsx`, `StartBox`: compositor independiente para iniciar sesiones. Debe recibir el mismo control de voz.
- `apps/desktop/src/components/StartingPanel.tsx`: compositor mientras arranca el proveedor; hay transición automática a la sesión. Necesita una política explícita para no perder dictados.
- `apps/desktop/src/shortcuts.ts`, `ipc.ts`, `store.ts` y `packages/contracts/src/index.ts`: atajos, comandos tipados, canales Tauri y estado existentes.
- `apps/desktop/src-tauri/src/commands/suggestions.rs`: peticiones auxiliares mediante programas oficiales; rama local con `Ollama::local().short_reply`. Tiene un timeout de 45 segundos para procesos externos: es infraestructura reutilizable, no una garantía de limpieza rápida.
- `crates/thingmaker-supervisor/src/agents/codex/bridge.rs`: anuncia imagen y contexto, pero no audio. `agents/gemini/bridge.rs` convierte el prompt a texto y conduce `agy` mediante stream-json. Que un proveedor soporte audio no significa que este puente ya lo transmita.
- `apps/desktop/src-tauri/src/commands/attachments.rs` ya acepta WAV/MP3; `acp/capabilities.rs` conoce `prompt_audio`. Son piezas parciales, no un dictado implementado.
- `.github/workflows/release.yml`: Mac universal **Intel + Apple silicon**, además de Windows. El brief inicial solo mencionaba Apple silicon; no hay que romper el paquete Intel.
- `tauri.windows.conf.json`: distribución NSIS. `tauri.release.conf.json`: firma Mac ad hoc. Permisos de micrófono y bibliotecas nuevas requieren validar el paquete instalado, no solo `tauri dev`.

### Evidencia local de proveedores

Se consultaron las ayudas de los programas, sin enviar audio ni iniciar turnos:

- `codex --version`: `codex-cli 0.167.0-alpha.4.1`.
- `claude --version`: `2.1.316 (Claude Code)`.
- `agy --help` y `agy acp --help`: existen modos print, stream-json y ACP, además de `mic-serve`. No se arrancó el micrófono.
- `codex app-server generate-json-schema --experimental --out <temporal>`: generación local del contrato, sin inferencia. Artefactos en `%TEMP%/thingmaker-voice-schema-20261116/`.

El esquema generado contiene:

```text
UserInput: { type: "localAudio", path: string }
UserInput: { type: "audio", url: string } o fileId
thread/realtime/start
thread/realtime/appendAudio
thread/realtime/stop
ThreadRealtimeTranscriptDeltaNotification
ThreadRealtimeTranscriptDoneNotification
```

Los contratos realtime están marcados experimentales. `clientManagedHandoffs` permite que el cliente controle las transferencias al agente; **no demuestra por sí solo que la sesión sea un transcriptor puro sin acciones**. Tampoco un schema demuestra disponibilidad del backend, cuota, soporte del modelo o permiso de la cuenta.

No se grabó audio, instalaron modelos, midieron latencias ni probaron permisos. No hay resultados de calidad propios. No se verificó una instalación de Ollama utilizable en esta sesión.

## 3. Audio a un modelo usando la suscripción

### Codex / ChatGPT

**Viabilidad: candidata a prueba, no descartada ni confirmada.** El esquema del binario instalado aporta evidencia más concreta que asumir que Codex solo acepta texto e imágenes. Es una versión alpha: no exigirla a todos los usuarios sin encontrar una versión soportada y reproducible.

Separar dos caminos:

- **Archivo al terminar:** grabar, enviar `localAudio` en un contexto auxiliar dedicado y pedir solo transcripción limpia. Es la primera prueba: menor complejidad, una respuesta final, sin parciales mientras se habla.
- **Realtime:** enviar PCM por el proceso oficial y recibir eventos de transcripción. Mayor integración y riesgo de que sea una conversación con un agente, no una operación de dictado. No conectarlo a la sesión de trabajo antes de demostrar que no dispara acciones.

La documentación pública de app-server describe la integración mediante el programa oficial [S1]. Una ruta diferente, Sign in with ChatGPT mediante token compartido, excluye audio y transcripción en sus limitaciones [S2]. **No extrapolar esa exclusión al binario instalado ni usarla como permiso para extraer credenciales.** ThingMaker debe continuar dejando el login y la comunicación con el proveedor al programa oficial, conforme a ADR-009.

Prueba de aceptación: audio de prueba no sensible → texto, con herramientas deshabilitadas por el contrato y no solo por un prompt; sin archivos de proyecto ni contexto de conversación; sin llamadas directas a endpoints privados; confirmar que no exige API key. Medir arranque, subida, primera salida y resultado completo. Una cuenta que rechaza audio no invalida el schema, pero sí invalida esa opción para ese usuario.

### Claude Code

Claude Code documenta dictado de micrófono a servidores de Anthropic usando una cuenta Claude.ai, con transcripción que no consume mensajes/tokens de Claude. Es una función de su interfaz [S3]. **No he encontrado en la documentación revisada un contrato de transcripción headless/ACP que podamos reutilizar directamente.**

No convertir la interfaz de terminal en una API mediante pulsaciones y lectura de pantalla. La limpieza de un texto ya transcrito sí puede usar el patrón de peticiones auxiliares existente, sujeto a su latencia y cuota. El hecho de que `/voice` sea rápido no prueba que lanzar un modelo general para limpiar lo sea.

### Antigravity y Gemini CLI

Antigravity documenta dictado con parciales y sin envío automático, pero lo limita a TUI interactiva y lo desactiva en `--print` [S4]. Esto afecta directamente a ThingMaker, cuyo puente usa print/stream-json. Su modo conversacional con Gemini Live [S5] tampoco equivale automáticamente a un RPC de transcripción.

Gemini CLI documenta lectura de audio y login con Google [S6]. Merece una prueba de archivo → transcripción limpia mediante el programa oficial. Sin embargo, **Gemini CLI y `agy` no son el mismo programa**: su disponibilidad no demuestra soporte en el puente actual. Primero verificar el contrato de audio del `agy` instalado o el coste explícito de añadir Gemini CLI; no sustituir el proveedor silenciosamente.

Para audio a un modelo general, el prompt debe exigir: conservar idioma, números, negaciones, nombres y restricciones; eliminar solo muletillas/repeticiones inequívocas; no responder ni ejecutar lo dictado. Conservar también versión literal cuando la interfaz permita obtenerla. Si solo existe un resultado limpio, evaluar su fidelidad contra el audio, no su fluidez.

**Latencia:** desconocida en estas cuentas. Para archivos: espera al fin de grabación + arranque/autenticación + subida + inferencia. Una transcripción final servida en streaming no da parciales durante la captura. La nube evita RAM de pesos locales, pero introduce red, cuotas y disponibilidad por cuenta.

## 4. Opciones locales y del sistema

| Opción | Ventaja | Coste o límite | Posición en la propuesta |
| --- | --- | --- | --- |
| Whisper.cpp multilingüe | Windows/Mac; CPU y aceleración; idioma forzable | Modelos grandes, consumo y latencia según equipo; parciales por ventanas | Comparador local principal |
| Whisper + Ollama para limpieza | Transcripción especializada y texto más cómodo; todo local | Dos modelos; arranque frío, cola/GPU y riesgo de reescritura | Evaluar limpieza por separado, opcional |
| Parakeet v3 + sherpa-onnx | Español/inglés, puntuación, CPU int8, mismo runtime | Paquete cercano a 640 MB; variante documentada offline | Comparador local principal |
| Audio directamente a Ollama | Reutiliza servicio ya integrado y evita empaquetar otro runtime | Requiere versión/modelo con audio verificado; endpoint por archivo | Candidato condicionado a compatibilidad real |
| Apple SpeechAnalyzer | API nativa para transcripción en vivo, recursos administrados por SO | macOS 26+, hardware/idiomas disponibles; puente Swift | Candidato preferente específico de Mac si pasa prueba |
| Apple SFSpeechRecognizer | Compatibilidad con sistemas anteriores | Offline depende de recognizer/idioma; autorización | Fallback solo comprobando soporte local |
| Windows.Media.SpeechRecognition | API del SO | Dictado libre documentado con reconocimiento online habilitado | No prometer local; probar español e integración Win32 |
| Windows AI Speech Recognition | Nueva transcripción local, streaming | Documentación la marca experimental, no producción | Vigilar, no base de primera entrega |
| Dictado del SO sobre textarea | Cero motor propio; utilidad inmediata | ThingMaker no controla eventos, limpieza o captura | Baseline manual de usabilidad |
| Web Speech API | Poco código aparente | Soporte/servicio variable en webviews y posible nube | No usar como backend de producción |

### Whisper, Parakeet y runtime

Whisper.cpp tiene soporte Windows y Apple silicon, Metal y backends CUDA/Vulkan [S7]. Comparar primero `small` multilingüe; usar `base` como escalón ligero si hace falta. No usar variantes `.en` para español. La tabla upstream da para small 466 MiB de disco y aproximadamente 852 MB de memoria, orientativos: medir el proceso real. CUDA no debe ser requisito del producto; habilitar aceleración adicional después de verificar una base CPU y Mac Metal. La licencia del runtime es MIT; registrar también la procedencia/licencia de los pesos elegidos.

Parakeet v3 publica WER FLEURS 3,45 % español y 4,85 % inglés, con puntuación/capitalización excluidas del cálculo [S8]. No son resultados del int8 de escritorio ni de instrucciones de programación. El paquete sherpa documentado ronda 640 MB y usa reconocimiento offline [S9]. Los parciales exigirían ventanas/VAD; no trasladar sin prueba el comportamiento del `.litertlm` stateful de Android. Runtime sherpa Apache-2.0 y modelo CC-BY-4.0: entregar atribución, procedencia y avisos de conversión.

Para Parakeet, preferir sherpa a ONNX Runtime directo: evita escribir extractor, tokenización y decoder TDT. Hay ejemplos Rust upstream [S10]. ONNX directo solo se justifica si resuelve un problema medido. No añadir Python/NeMo completo al instalador.

### Las dos interpretaciones de «Whisper con Ollama»

1. **Whisper transcribe y Ollama limpia texto.** Viable arquitectónicamente con piezas existentes. Añade una segunda inferencia. Probar un modelo pequeño multilingüe instalado, sin razonamiento, con límite de salida; no asumir que hace falta uno de 7B.
2. **Ollama recibe directamente audio.** Su documentación actual incluye `/v1/audio/transcriptions` y eventos de salida; procesa archivos por tramos [S11]. Por tanto sería incorrecto afirmar que Ollama solo maneja texto. Eso no acredita un modelo Whisper concreto, la versión mínima instalada o streaming de entrada de micrófono. Verificar versión publicada, modelo exacto, licencia, capacidad de audio, español y ejecución sin nube. No recomendar un modelo comunitario solo por llamarse `whisper`.

Las páginas actuales de Gemma 3n describen capacidades de audio del modelo, pero eso por sí solo tampoco acredita la ruta de audio de cada tag de Ollama [S12]. La prueba debe demostrar audio → texto, no limitarse a que `ollama pull` tenga éxito.

### Nativo en macOS y Windows

SpeechAnalyzer/SpeechTranscriber es una alternativa real, no una nota al margen: Apple describe resultados en vivo y modelos gestionados por el sistema [S13]. `supportedLocales` está disponible desde macOS 26 y puede devolver vacío en hardware no soportado [S14]. Consultar disponibilidad de español/inglés y descargar assets antes de grabar. Medir cambio de idioma, terminología técnica y Mac Intel; conservar fallback para las máquinas que no lo soporten. Tamaño/RAM dependen de assets del SO: no se han medido.

Con SFSpeechRecognizer, exigir `supportsOnDeviceRecognition` y `requiresOnDeviceRecognition`; si falta soporte, informar en vez de mandar audio a la nube sin elección explícita [S15]. Necesita autorización Speech además del micrófono según la API empleada.

En Windows, la API clásica documenta que el dictado necesita reconocimiento online habilitado [S16]. No confundir gramáticas de comandos locales, Voice Access, Win+H y una API embebible. La nueva Windows AI Speech Recognition documenta CPU/NPU, Windows 11 24H2+, SDK experimental y requisitos de empaquetado, y avisa que no es apta para producción [S17]. No rediseñar la distribución NSIS alrededor de ella para este MVP. Calidad/latencia en español de ambas vías quedan por medir.

Web Speech: Safari tiene reconocimiento basado en Siri [S18], pero eso no prueba idéntico soporte en WKWebView. WebView2 tiene reportes de fallos por versión/servicio [S19]. No afirmar «no existe»: afirmar que no hemos establecido un contrato portable, offline y fiable para ambos webviews. `getUserMedia` para capturar audio es una cuestión separada de Web Speech para reconocerlo.

## 5. Diseño común de la experiencia

**Activar → hablar → detener → revisar → enviar.** Botón junto al compositor y atajo solo con la ventana activa; candidato Ctrl/Cmd+Shift+D, pendiente de comprobar conflictos de plataforma. Toggle por defecto, Escape cancela. No interpretar «envía», «borra» o «acepta» como acciones mientras se dicta contenido. Las aprobaciones actuales siguen siendo explícitas.

Esto permite dirigir al agente hablando, pero no es un modo completamente manos libres. Los comandos de navegación/envío por voz y la respuesta hablada serían un alcance posterior, no una capacidad prometida por el MVP.

- Mismo control en el compositor de sesión y en StartBox. Durante el arranque, conservar un identificador de destino estable o deshabilitar temporalmente voz; nunca insertar el resultado en otra sesión.
- Mostrar backend y destino, estado de preparación, nivel de micrófono, duración, transcripción provisional y botón de cancelar.
- La captura comienza solo al mostrar «Escuchando»; preparación de modelo no puede perder las primeras palabras silenciosamente.
- Vista previa separada dentro del área del compositor. El borrador no se reemplaza en cada parcial. Al iniciar se guarda revisión del borrador y selección; al terminar se inserta en esa selección **solo si la revisión sigue siendo la misma**.
- Se permite escribir durante el dictado: si el borrador cambió, conservar el texto reconocido en una bandeja «Insertar dictado» en vez de sobrescribir. Insertar es una operación de deshacer. Mover el cursor no cambia retrospectivamente el destino capturado.
- Al cambiar de sesión, ocultar la ventana o suspender: detener captura; conservar resultado para el destino original, sin enviarlo. Escape descarta la nueva transcripción y deja intacto el borrador.
- Mientras finaliza, Enviar no despacha un parcial accidentalmente. Una vez insertado, Enviar/Encolar/Dirigir usan las acciones existentes.
- Límite inicial propuesto: 2 minutos por dictado, visible. Parar a tiempo debe finalizar y conservar texto, no desecharlo. Ampliable tras medir.

Limpieza: ajuste independiente `ninguna | local | proveedor`. Mostrar primero el texto literal cuando esté disponible. La limpieza nunca es necesaria para poder enviar; timeout propuesto de 5 s en caliente, con resultado literal disponible si expira. No eliminar negaciones ni «corregir» rutas, cifras o nombres por reglas. No aplicar un resultado tardío si el usuario ya editó/envió el borrador. Un modelo local que limpie texto no debe tener herramientas.

## 6. Captura, arquitectura y contratos

Recomiendo captura nativa Rust con CPAL [S20], no audio transportado a través del DOM. Enumerar dispositivo y configuración: no asumir que el micro ofrece 16 kHz. Capturar en su frecuencia real, convertir canales y remuestrear fuera del callback; formato interno mono 16 kHz, adaptar a lo exigido por cada backend. Callback sin inferencia, disco, bloqueos o red; cola acotada y error visible ante overflow.

`getUserMedia` es válido para un prototipo, pero añade permisos del WebView, negociación de codecs y transferencia de PCM por IPC. Con captura Rust no hace falta permiso de micrófono WebView2: sí permiso de Windows para apps de escritorio. En Mac incluir `NSMicrophoneUsageDescription`, autorización nativa y entitlements correspondientes al modo de firma/sandbox [S21]. Probar desde la aplicación instalada.

Distribución propuesta:

- `crates/thingmaker-supervisor/src/voice/`: estado, selección de backend, límites, cancelación, procesos y eventos; contratos testeables sin Tauri.
- `apps/desktop/src-tauri/src/commands/voice.rs`: capa estrecha, comprobación de ventana/destino y canales dirigidos al cliente propietario. Los comandos no se añaden al allowlist remoto ni a herramientas de agentes.
- Si gana un motor embebido, una crate `thingmaker-voice-worker` separada para inferencia y dependencias nativas. Proceso persistente por stdio, pesos cargados una vez. Más trabajo de empaquetado, pero permite liberar RAM al cerrarlo y aísla fallos nativos.
- Captura en el host; worker recibe PCM con framing binario acotado, devuelve eventos JSON; no mezclar logs con stdout. Sin servidor HTTP para el motor embebido. Tauri documenta binarios auxiliares por plataforma [S22]. Alternativa en proceso solo si el prototipo prueba carga/descarga y el ahorro de empaquetado compensa el riesgo.
- Si gana Ollama, usar el cliente loopback existente; si gana suscripción, usar proceso oficial autenticado y contexto auxiliar sin herramientas. Ningún backend se inserta en el historial de trabajo por defecto.

Comandos propuestos: `voice_status`, `voice_devices`, `voice_prepare`, `voice_start`, `voice_stop`, `voice_cancel`, `voice_release`. Una sola captura global. `start` devuelve `dictationId`; cada evento incluye id, secuencia y destino. Eventos: `state`, `level`, `partial(revision,text)`, `segmentFinal(segmentId,text)`, `done(text)`, `error(code)`. Actualizaciones parciales reemplazan su revisión; finales son idempotentes. Cancelar invalida eventos tardíos. Canales con límites; nivel máximo 10 Hz y texto provisional máximo 4 Hz.

Para motores offline: VAD, pausa objetivo de 500–800 ms, preview como máximo cada 1–1,5 s y tramos limitados a 10–15 s. Son parámetros de prueba. Una frase larga sin pausas requiere solape y reconciliación comprobada, no concatenar cada ventana. No afirmar coste lineal si se retranscribe repetidamente un buffer creciente. Si la cola se atrasa, omitir previews y mantener el audio final; no descartar muestras a escondidas.

Memoria: cargar al activar o preparar explícitamente, mantener unos 3 minutos tras el uso y liberar. Ningún modelo ni micrófono se carga al arrancar ThingMaker si voz está desactivada. Audio en RAM, sin persistencia ni logs de contenido; retención solo durante la operación. Limitar longitud de texto y duración también en Rust.

Descarga local: opt-in, progreso/cancelación, manifest de versión/tamaño/SHA-256/procedencia/licencia, fichero temporal y rename atómico tras verificar. Rutas de extracción validadas. Pesos en app-data, nunca en el repo; no bajar 640 MB dentro del instalador por defecto. El peso de runtime/DLLs se medirá en el prototipo. Fijar versiones exactas al implementar, conforme a Cargo.toml; no dependencias `latest`.

## 7. Encaje con decisiones existentes

- ADR-001: UI React y puente Tauri se conservan.
- ADR-003: micrófono, procesos y validación en Rust; frontend recibe texto/eventos acotados. El supervisor no se convierte en un mero proxy de órdenes arbitrarias.
- ADR-004: worker por stdio, sin listener nuevo. Ollama ya constituye una integración local explícita; documentar esa dependencia, no presentar localhost como ausencia total de red.
- ADR-009: procesos oficiales con login propio; no extraer tokens, no endpoints privados ni claves de API. Limpiar mediante una API cloud por clave entraría en conflicto. Una nueva interfaz oficial soportada puede evaluarse sin cambiar este principio.
- Local Models: dictado local mantiene audio/texto local hasta que el usuario decide enviarlo a un agente cloud. Una limpieza cloud debe indicarse como tal, incluso si el reconocimiento fue local.
- ADR-012: no cambia. No hay comandos, permisos o UI nuevos para Android.

## 8. Cambios previstos por módulo

| Ruta | Cambio cuando se implemente |
| --- | --- |
| `packages/contracts/src/index.ts` | Configuración, estados, eventos y nombres de comandos |
| `apps/desktop/src/ipc.ts` | Wrappers tipados y canal de voz |
| `apps/desktop/src/voice.ts` (nuevo) | Control de captura, revisión de borrador, cancelación y eventos tardíos |
| `components/VoiceInput.tsx` (nuevo) | Botón, preview y estado compartidos |
| `components/SessionPanel.tsx` | Integración con draft/selección y envío/steering actuales |
| `components/WorkspaceView.tsx`, `StartingPanel.tsx` | Nuevo prompt y transición de arranque sin pérdida |
| `shortcuts.ts`, `SettingsPanel.tsx`, `i18n/` | Atajo, backend, dispositivos, idioma soportado y mensajes EN/ES |
| `src-tauri/src/commands/voice.rs`, `commands/mod.rs`, `state.rs`, `lib.rs` | Comandos, lifecycle y limpieza al ocultar/salir |
| `crates/thingmaker-supervisor/src/voice/` (nuevo) | Coordinación y adaptador del backend seleccionado |
| `commands/suggestions.rs` | Extraer solo infraestructura útil para limpieza, sin reutilizar prompt de sugerencias |
| `Cargo.toml`, manifiestos Tauri, `.github/workflows/release.yml` | Dependencias fijadas, permisos y worker si gana motor embebido |
| `docs/` | Instalación/modelos, atribución y decisión arquitectónica |

No es necesario ampliar las capacidades de audio de todos los agentes: el servicio de dictado devuelve texto utilizable con cualquiera. Solo se adapta el proveedor que actúe como transcriptor.

## 9. Prueba comparativa y criterio de elección

Antes de escribir integración de producto, usar un corpus no sensible de 30–40 clips de 5, 20 y 60 s: español, inglés y mezcla; rutas, nombres de código, cifras, negaciones, rectificaciones, silencios y ruido. Incluir «no hagas commit», «cambia solo este archivo» y correcciones de números. Audio sintético sirve para contratos; la elección de calidad exige voz humana.

Ejecutar primero Windows CPU sin GPU dedicada como referencia de producto y Mac Apple silicon; añadir Mac Intel para compatibilidad. Registrar OS/CPU/RAM/micrófono, versión exacta de runtime, modelo, hash y cuenta/plan sin credenciales. La RTX del usuario no define requisitos mínimos.

Medir por separado:

- Frío: primer uso con pesos en disco pero descargados de memoria. Caliente: repeticiones con proceso/modelo preparado.
- Tiempo desde fin de habla hasta texto literal y hasta limpio; primer parcial desde inicio del habla y número de revisiones.
- WER normalizado, preservación exacta de términos críticos y tiempo humano de corrección. Evaluar limpieza contra intención, no solo contra WER.
- Pico/residuo de RAM, carga CPU/GPU, tamaño de instalación y descarga; cola al convivir con un agente local.
- Desconexión, cuota, cancelación, silencio, permiso denegado y retorno de un resultado tarde.

**Objetivos de producto propuestos, no mediciones:** para clips de 20 s, final literal p95 ≤2 s local caliente o ≤4 s en vía con suscripción; texto limpio p95 ≤5 s; preview ≤2 s donde el backend lo permita. Cero envío automático, cero ejecución durante transcripción y cero sobrescrituras del borrador en pruebas adversarias. Si un candidato solo da final, puede pasar como primera versión siempre que se presente así.

Aplicar gates en este orden: interfaz oficial utilizable y sin herramientas → calidad/fidelidad → latencia → distribución/mantenimiento. No elegir el más rápido si pierde restricciones. Si suscripción necesita endpoints privados, automatizar TUI o una alpha exclusiva, no será el backend por defecto. Si ninguno cumple latencia, mantener final-only y ajustar el alcance explícitamente antes de integrar.

## 10. Fases y esfuerzo estimado

Estimaciones de trabajo de ingeniería, no fechas comprometidas; incluyen una persona con acceso a ambos sistemas.

| Fase | Entregable | Esfuerzo |
| --- | --- | --- |
| A. Viabilidad comparada | Prueba archivo Codex, dos motores locales, limpieza y nativo Mac; matriz con descartes y medidas | 2–4 días |
| B. Flujo común | Captura, botón/atajo, estados, borrador seguro, cancelación; backend simulado | 2–3 días |
| C. Un backend real | Adaptador ganador y ajustes; sin añadir todos los candidatos | 2–4 días con servicio existente; 4–7 con worker/modelos empaquetados |
| D. Validación/distribución | Windows/Mac instalados, transiciones, fallos, licencias y CI | 2–4 días |

Total razonable: aproximadamente **8–15 días** si gana una interfaz de servicio ya disponible; **10–18 días** con motor local empaquetado. Un segundo backend de producción o comandos manos libres son ampliaciones. Reservar trabajo adicional si Mac Intel o distribución de librerías nativas obliga a cambiar runtime. No bloquear todas las fases investigando indefinidamente cada modelo nuevo.

Pruebas automatizadas: máquina de estados, orden/deduplicación de eventos, cancelación, stop doble, overflow, remuestreo, downloads corruptas, cambio de draft/destino, Unicode/selección, transición StartingPanel, cero `send/enqueue/steer` antes de acción explícita y limpieza tardía. Backend simulado sin red/modelos en tests normales; corpus real en pruebas opt-in.

Validaciones de integración previstas: `pnpm typecheck`, `pnpm test`, `pnpm rust:test` y pruebas dirigidas del host (`cargo test -p thingmaker-desktop`) cuando se implemente. Una suite de supervisor no valida permisos Tauri. Pruebas manuales: permisos iniciales/revocación, USB/Bluetooth, suspensión, bandeja, red caída/cuota, dos ventanas y versión instalada de Windows/Mac. El soporte Intel requiere prueba, no solo compilación universal.

## 11. Decisiones abiertas

No bloquean esta investigación; defaults propuestos para una implementación posterior:

- ¿Audio local obligatorio o puede salir por la suscripción? La petición más reciente autoriza investigar ambas opciones; la UI debe pedir elección explícita de backend cloud, sin fallback oculto.
- ¿Se acepta texto solo al terminar si evita una descarga y tiene buena latencia? Propuesta: sí para el primer backend, dejando visible «Transcribiendo».
- ¿Basta dictado más envío manual, o se exige cero teclado/ratón? Propuesta inicial: dictado y envío explícito; el modo totalmente manos libres necesita alcance propio.
- ¿Priorizar Windows para una beta personal? Propuesta: medir Windows primero y no declarar soporte Mac sin probarlo.

La siguiente acción técnica propuesta es la fase A, no integrar Parakeet por inercia. Esta investigación documental y de contratos reduce candidatos; todavía no sustituye esa comparación de audio.

## Fuentes primarias consultadas

Consultadas el 2026-11-16. Las URLs vivas y versiones experimentales pueden cambiar.

- S1: [Codex app-server](https://learn.chatgpt.com/docs/app-server), complementado con el schema generado del binario local indicado arriba.
- S2: [Límites de Sign in with ChatGPT, ruta token-sharing](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).
- S3: [Claude Code: voice dictation](https://code.claude.com/docs/en/voice-dictation).
- S4: [Antigravity CLI: dictado y restricciones print](https://antigravity.google/docs/cli/commands/voice/).
- S5: [Antigravity voice mode](https://antigravity.google/docs/voice-mode/), [headless](https://antigravity.google/docs/cli/headless/), [ACP](https://antigravity.google/docs/cli/acp/).
- S6: [Gemini CLI: archivos y audio](https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/tools/file-system.md), [autenticación](https://geminicli.com/docs/get-started/authentication/).
- S7: [whisper.cpp](https://github.com/ggml-org/whisper.cpp).
- S8: [Ficha NVIDIA Parakeet v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3).
- S9: [sherpa: modelos NeMo offline](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html), [licencia](https://github.com/k2-fsa/sherpa-onnx/blob/master/LICENSE).
- S10: [Ejemplos Rust de sherpa](https://github.com/k2-fsa/sherpa-onnx/tree/master/rust-api-examples).
- S11: [Ollama: API compatible, audio/transcriptions](https://docs.ollama.com/api/openai-compatibility).
- S12: [Gemma 3n en Ollama](https://ollama.com/library/gemma3n).
- S13: [Apple WWDC25: SpeechAnalyzer](https://developer.apple.com/videos/play/wwdc2025/277/).
- S14: [SpeechTranscriber.supportedLocales](https://developer.apple.com/documentation/speech/speechtranscriber/supportedlocales.md).
- S15: [Apple requiresOnDeviceRecognition](https://developer.apple.com/documentation/speech/sfspeechrecognitionrequest/requiresondevicerecognition), [autorización Speech](https://developer.apple.com/documentation/Speech/asking-permission-to-use-speech-recognition).
- S16: [Windows.Media.SpeechRecognition](https://learn.microsoft.com/en-us/windows/uwp/ui-input/speech-recognition).
- S17: [Windows AI Speech Recognition](https://learn.microsoft.com/en-us/windows/ai/apis/speech-recognition).
- S18: [WebKit: Speech Recognition en Safari](https://webkit.org/blog/11648/new-webkit-features-in-safari-14-1/).
- S19: [WebView2: fallo de SpeechRecognition por versión](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5724). Reporte de incidencia, no contrato de compatibilidad.
- S20: [CPAL](https://docs.rs/cpal/latest/cpal/). La documentación consultada muestra 0.18.2; verificar MSRV y fijar versión al implementar.
- S21: [Apple: autorización de captura](https://developer.apple.com/documentation/bundleresources/requesting-authorization-for-media-capture-on-macos).
- S22: [Tauri: binarios auxiliares](https://v2.tauri.app/develop/sidecar/).
