# Registro de cambios

Los cambios relevantes se anotan aquí por versión. Antes de la primera release, los cambios
pendientes se agrupan en `Unreleased`; las versiones publicadas usarán etiquetas Git y notas de
release.

## Unreleased

### Renombrado

- CodeLoom pasa a llamarse **Loomcoding**; la crate `batten`, a `loomcoding-cloth`. Cambian los
  paquetes, el binario `codeloom` (ahora `loomcoding`), `loomcoding.toml`, `LOOMCODING_*`,
  `~/.loomcoding/` y el plugin `loomcoding.herdr`. Sin capa de compatibilidad: véase la sección
  de migración del README.

### Preparación del proyecto

- Añadidas pautas de contribución, seguridad, conducta y licencias.
- Preparada CI para los checks del workspace en Linux y Windows.
- Ignorados los ficheros `.env.*` además de `.env`.
