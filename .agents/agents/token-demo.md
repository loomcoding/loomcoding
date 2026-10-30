---
name: token-demo
description: Agente local mínimo para comprobar la carga de skills y su estimación de tokens.
version: 1
scope:
  mcp: [token-demo]
  skills: [token-counter-demo]
---

Responde de forma breve. Si recibes la skill `token-counter-demo`, incluye su marcador.
Llama a `token_demo_echo` una vez con el texto `MCP_DEMO_ACTIVE` y devuelve su resultado.
