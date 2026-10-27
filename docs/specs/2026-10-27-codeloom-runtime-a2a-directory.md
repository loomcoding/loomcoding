# Entrada A2A compartida del runtime

> Estado: aprobada para implementación como R2.

## Objetivo

Exponer una tarjeta A2A por agente vivo y enrutar las operaciones del protocolo hacia el agente indicado por `AgentInterface.tenant` en un único listener propiedad del runtime.

## Contrato

- Cada tarjeta se obtiene desde una URL de descubrimiento directa, por agente. Su cuerpo es un AgentCard A2A estándar; la interfaz declara JSON-RPC v1.0, el endpoint compartido y el tenant opaco del agente.
- La ruta HTTP de la tarjeta es una decisión de hosting. No añade operaciones JSON-RPC ni campos A2A. No se publica una operación de listado de agentes.
- El listener solo arranca desde configuración de runtime y con autenticación válida. No hereda el puerto, URL pública ni secretos del primer proyecto que abra una TUI.
- Configuración en `~/.codeloom/runtime.toml`, bajo `[a2a]`; solo `role = "serve"` o `"both"` abre el listener. La tarjeta de cada tenant vivo se obtiene en `/agents/{tenant}/.well-known/agent-card.json`.
- Los IDs de tenant son IDs del runtime, se validan contra el registro y no autorizan por sí mismos. La autenticación y política local siguen aplicándose.
- Una tarjeta se mantiene mientras el agente siga registrado en el runtime, aunque su TUI se desconecte; se retira al cerrar el runtime.
- No publicar rutas locales, prompts, transcripts, secretos ni la marca `internal` en la tarjeta.

## Aceptación

1. El servidor sirve AgentCards distintas para dos agentes vivos del mismo proyecto.
2. Cada tarjeta anuncia el tenant correcto y la URL compartida.
3. La tarjeta se obtiene por una URL directa, sin método A2A nuevo.
4. El listener requiere autenticación y no se enlaza a interfaces públicas por defecto.
5. Un tenant inexistente o una tarjeta retirada no revela si otros agentes existen.
