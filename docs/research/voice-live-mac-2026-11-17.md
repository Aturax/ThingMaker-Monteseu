# Transcripción provisional — validación en Mac del 2026-11-18

Implementación iniciada el 17 de noviembre y validación de interfaz terminada el 18. Rama `codex/desktop-voice-plan`, sobre `1c0840c`, con cambios locales de esta ampliación. MacBook Pro M4 Pro, macOS 26.6.2, CPU, micrófono integrado a 48 kHz. No se hizo commit/push de esta ampliación.

## Comportamiento implementado

- Tras reunir dos segundos de audio se solicita la primera vista previa. Se solicitan actualizaciones aproximadamente cada dos segundos, con el tiempo de inferencia añadido a la entrega.
- La captura y la inferencia usan hilos distintos. Un único reconocedor procesa secuencialmente las vistas previas y la pasada final. El callback de audio conserva su anillo preasignado sin asignaciones ni mutex.
- Cada vista previa procesa como máximo los últimos 16 s. La unión conserva el texto anterior fuera de la ventana y revisa el tramo reciente con solape. Solo puede haber una vista previa en vuelo; si el motor tarda, se saltan solicitudes en vez de acumular trabajo.
- El texto provisional aparece en el recuadro de revisión, de solo lectura. Insertar y Usar original permanecen deshabilitados mientras sea provisional y durante la pasada final. El borrador se puede seguir editando independientemente.
- Al detener, se suprimen resultados provisionales tardíos y se vuelve a transcribir toda la grabación acotada. Esa pasada no utiliza el empalme provisional. El resultado final sustituye a la vista previa y habilita edición e inserción explícita. Solo el resultado final puede pasar a la limpieza opcional.
- Cancelar o abandonar el compositor descarta las vistas previas; los resultados de una operación anterior no pueden modificar una nueva. La cancelación también termina el hilo de inferencia después de la operación nativa que ya estuviera en curso.

## Cadencia medida con micrófono real

Probe de 18 s realizado el 17 de noviembre, justo antes de pausar el trabajo. Audio sintético español reproducido por los altavoces; el probe usa el mismo Service nativo que la app y no guarda audio ni imprime transcripciones.

- Primera vista previa: **2.177 ms** desde el inicio de grabación (2,18 s).
- 8 vistas previas; intervalos: **2.150, 2.182, 2.124, 2.163, 2.117, 2.213 y 2.082 ms**.
- Mediana del intervalo: **2.150 ms (2,15 s)**.
- Frase esperada «No implementes todavía los cambios»: comprobación correcta en el resultado final.
- Cero errores fatales, cero avisos del host, 865.280 muestras capturadas. No se presenta como distribución de latencia ni p95.

## Grabación sintética de dos minutos

Prueba opt-in con el modelo local, usando el corpus sintético guardado y repetido hasta 120 s. Se suministran instantáneas de audio a 2, 4…120 s (sin esperar tiempo real entre ellas):

- 60 vistas previas, cada solicitud acotada a 16 s.
- Primera inferencia de 2 s: aproximadamente 145 ms; ventanas largas, aproximadamente 1 s en este binario debug. Había otras tareas de compilación; estos tiempos no son un benchmark de producción.
- Última vista previa: 2.109 caracteres. Resultado final: 2.116 caracteres, idéntico a una transcripción independiente del audio completo.
- La prueba detectó una unión incorrecta cuando se buscaban coincidencias en toda la historia de frases repetidas. Se corrigió separando el texto anterior y la ventana reciente. Regresiones específicas cubren frases repetidas y repeticiones legítimas después de silencio.
- La pasada final de 120 s tardó unos 8,9 s en este entorno de prueba. El límite de dos segundos es una cadencia objetivo para las vistas previas, no una garantía de latencia final ni de funcionamiento en cualquier hardware.

## Comprobación integrada en la app optimizada, 18 de noviembre

Se cerró la versión anterior y se abrió el paquete nuevo. El usuario concedió el permiso de micrófono que macOS solicitó al cambiar el ejecutable. Se comprobó:

1. Aparición del recuadro mientras seguía activa la grabación, con la indicación de texto provisional, textarea no editable e inserción deshabilitada.
2. Actualizaciones del contenido antes de detener. Se modificó el borrador mientras la vista previa seguía cambiando: el borrador se conservó.
3. Al pulsar Detener, se mostró Transcribing y el texto siguió sin poder insertarse hasta terminar.
4. La pasada final sustituyó el contenido por «Cambia solo el color del botón, no modifiques ningún otro archivo.»; desapareció la indicación provisional, el campo pasó a editable e Insertar se habilitó.
5. Se corrigió manualmente la puntuación del resultado final, se cambió otra vez el borrador y se insertó el texto revisado: el nuevo borrador se conservó y el dictado se añadió. Deshacer restauró exactamente ese borrador.
6. Se eliminó el borrador de prueba y la app quedó inactiva, sin grabación ni reproducción activas. No se pulsó Enviar ni se inició ninguna sesión con el contenido dictado.

El usuario informó de un vídeo de fondo. La toma larga contenía ese audio y mostró errores provisionales y mezcla de idiomas; no se usa para afirmar precisión con voz humana. La comprobación final se repitió con un WAV sintético corto. No se guardó audio ni se copiaron a este informe las transcripciones del vídeo. No se envió audio a proveedores ni se activó limpieza cloud. Los ajustes de sonido del Mac se dejaron como estaban al reanudar (altavoces activos al 31 %).

## Pruebas y paquete

- `pnpm typecheck`: correcto.
- Suite frontend: **633 desktop + 2 contratos**, sin fallos.
- `cargo test --workspace`: **510 pasan, 9 omitidos, cero fallos**. Una de las omitidas es la prueba de dos minutos con modelo; se ejecutó explícitamente y pasó.
- Clippy del módulo de voz con todos sus targets y de la biblioteca del host: correcto.
- Regresiones: borrador protegido, provisional de solo lectura, final independiente, final vacío, limpieza solo del final, cancelación, resultados tardíos y cola sin acumulación con un decodificador lento.
- Firma local comprobada con `codesign --verify --deep --strict`.
- App ARM optimizada: `target/release/bundle/macos/ThingMaker Dev.app`.
- ZIP actualizado: `output/ThingMaker-Voice-Test-mac-arm64.zip`.
- SHA-256 del ejecutable probado: `c1f14589d1a43bb9125f3551d0a4e79b03d9879d4e20965e2de564bbfe7f48c3`.

Logs sin audio ni transcripciones y resumen de tiempos: `output/voice-validation-mac/live/` (ignorado por Git). La vista previa sigue siendo provisional; la calidad en ruido, distintos idiomas y otros dispositivos requiere más muestras. No se ha validado Intel ni se ha publicado una versión.
