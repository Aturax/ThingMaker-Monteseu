# Integración de web-mcp con los agentes locales

El PR #15 contra main introduce la elección de Codex o Claude Code para Ollama y la selección de skills y servidores MCP por agente. El PR #14 integra OpenCode Browse. Al actualizar la rama de web-mcp, ten en cuenta estos puntos.

- Usa el proveedor local para el modelo y el harness elegido para el protocolo. Un agente local puede ser un lanzamiento Codex o Claude. Conserva el identificador del modelo y la selección al reanudar la sesión.
- Registra ThingMaker Web como un MCP seleccionable en el catálogo. Para una selección personalizada, inicia la conexión únicamente cuando su ID esté marcado. La skill web-scout puede sugerir ese MCP. Marcar la skill por sí sola debe conservar la elección de conexiones del usuario.
- Mantén la búsqueda nativa de Codex desactivada cuando el modelo es Ollama. La búsqueda de ThingMaker Web viaja por su conexión MCP autorizada. Un caso de regresión es preguntar por una previsión actual con Gemma, cerrar y reanudar ese mismo chat, y comprobar web_search y web_read.
- En Windows, conserva el puente stdio local en los lanzamientos de sesión y de trabajadores. El PR #15 empaqueta Node y el SDK de MCP bajo local-mcp-runtime. Al combinar los hooks de desarrollo y build, incluye tanto local-mcp-runtime como web-runtime y remote-ui. Si más adelante compartes Node entre ambos, comprueba también el arranque de los MCPs locales sin una sesión web.
- El shim de stdio de Windows y la excepción de ACL para Program Files aparecen en ambas ramas. Al integrar los cambios basta con una copia. Comprueba el instalador y una llamada a un MCP stdio con cada harness.

[PR #15 · Harness y recursos de agentes locales](https://github.com/Monteseu/ThingMaker/pull/15) · [PR #14 · Integración de Browse](https://github.com/Monteseu/ThingMaker/pull/14)
