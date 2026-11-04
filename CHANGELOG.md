# Registro de cambios

Los cambios relevantes se anotan aquí por versión. Antes de la primera release, los cambios
pendientes se agrupan en `Unreleased`; las versiones publicadas usarán etiquetas Git y notas de
release.

## Unreleased

### Servidores locales

- Nuevas claves opcionales `[provider] context_window` y `request_timeout_secs` en `loomcoding.toml`
  (la segunda, de 1 a 3600 s; por defecto 120 s en el adaptador OpenAI-compatible).
- Reconocido el rechazo por exceso de contexto de llama.cpp, que ahora provoca un resumen y un reintento.
- Los errores HTTP del adaptador OpenAI-compatible incluyen el cuerpo de la respuesta, acotado a 512 caracteres.

### Renombrado

- CodeLoom pasa a llamarse **Loomcoding**; la crate `batten`, a `loomcoding-cloth`. Cambian los
  paquetes, el binario `codeloom` (ahora `loomcoding`), `loomcoding.toml`, `LOOMCODING_*`,
  `~/.loomcoding/` y el plugin `loomcoding.herdr`. Sin capa de compatibilidad: véase la sección
  de migración del README.

### Preparación del proyecto

- Añadidas pautas de contribución, seguridad, conducta y licencias.
- Preparada CI para los checks del workspace en Linux y Windows.
- Ignorados los ficheros `.env.*` además de `.env`.
