# Plan — Catálogo externo de agentes y skills como perfiles locales

> Estado: hecho (2026-11-01). Tareas 1 a 6 cerradas.
> Spec: `docs/specs/2026-11-01-codeloom-external-catalog-profiles.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real: proveedor simulado. Ninguna prueba lee el catálogo real del usuario: todo con directorios temporales.
- `codeloom-core` no depende de `ratatui` ni `crossterm`.
- Sin `catalog_roots`, comportamiento y pruebas existentes idénticos.

## Tareas

1. **Configuración.** Pruebas primero: `[agents] catalog_roots` se lee, admite `~`, y se acota a 8. Luego el campo en
   `AgentsConfig`.
2. **Nivel `Catalog` y carga.** Pruebas primero (criterios 1, 2, 6, 7): una fuente de ficheros por raíz
   (`<raíz>/.agents/agents`), nivel `Catalog`, siempre pendiente, precedencia y tope de 200 ficheros. Luego `ProfileLevel::Catalog`
   y su alta en el registro (`registry/source.rs`, `registry/mod.rs`).
3. **Aprobación y prompt.** Pruebas primero (criterio 3): aprobado aparece con `(catalog)`, `agent_origin` =
   `catalog:<ruta>`. Luego `prompt_section` y el descriptor.
4. **Skills y huella.** Pruebas primero (criterios 4 y 5): la skill del catálogo solo se resuelve para su perfil; la huella
   incluye el fichero y las skills citadas. Luego la resolución en el lanzador (`load_scoped_skills` con raíz extra) y
   `content_hash` ampliado para el nivel `Catalog`.
5. **Interfaz.** Pruebas primero (criterio 8): `agent_tag` con `·catalog`; `/agents` agrupa los de catálogo. Luego el código
   en `codeloom-tui`.
6. **Cierre.** Cuatro comprobaciones, actualizar la spec del registro (nuevo nivel) y el inventario, nota en el PoC de qué
   ruta poner en `catalog_roots` (sin editar el PoC). Commit.
