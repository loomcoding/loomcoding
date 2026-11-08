# Ayuda y documentación de usuario — plan

> **Para quien ejecute:** una tarea cada vez; pruebas primero (rojo visible); los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`); un commit por
> tarea. Cargo desde PowerShell. Rama `ayuda`. Binario real solo con `LOOMCODING_HOME` temporal.

**Spec:** `docs/specs/2026-11-08-loomcoding-ayuda-y-documentacion-de-usuario.md`. Las reglas de estilo de la spec son
obligatorias y las comprueba la tarea 1.

## Restricciones globales

- **Bilingüe:** cada página se escribe en `docs/user/en/` (referencia) y en `docs/user/es/` (espejo), mismas rutas y mismos
  títulos de sección. `docs/user/README.md` en inglés. Tono neutro y claro; nada de jerga regional.
- Texto de usuario: párrafos ≤ 4 líneas, esquemas, ejemplo primero, sin datos personales, valor por defecto y «requiere
  reiniciar» en cada ajuste.
- Las páginas de usuario viven en `docs/user/` y no copian la documentación técnica.
- Sin dependencias nuevas salvo la de la tarea 7 (sitio web), que se decide entonces.
- Una captura sin su script generador no entra al repo.

## Entrega 1 — Documentación de usuario

### Tarea 1 — Guía de estilo, plantilla y guardianes
**Ficheros:** `docs/user/README.md` (índice y guía de estilo, en inglés, enlazando a `en/` y `es/`), `docs/user/_plantilla.md`, una prueba en
`crates/loomcoding-cli/tests/docs_usuario.rs` que recorre `docs/user/{en,es}/**/*.md` (excepto `_plantilla.md`).
- [ ] Pruebas (rojo primero): falla con un párrafo de más de 4 líneas, un enlace relativo roto, un dato personal
  (`C:\Users\`, `/home/<usuario>`, un correo), una página sin la apertura *Qué es · Para qué · Cómo*, una imagen sin texto
  alternativo, y una página que existe en un idioma y no en el otro; pasa con la plantilla rellenada en los dos. Cada fallo dice fichero y línea.

### Tarea 2 — Empezar y conceptos
**Ficheros:** `docs/user/empezar/{instalar,primer-arranque,proveedor-nube,proveedor-local,primer-prompt}.md` y
`docs/user/conceptos/{agente-subagentes-jobs,sesiones,contexto-y-compactacion,permisos,flujos-odd,a2a}.md`.
- Con diagramas Mermaid donde ahorren texto (el árbol agente → subagentes → jobs; el ciclo de un turno).
- Cada comando de ejemplo se ha ejecutado de verdad con `LOOMCODING_HOME` temporal; quien escribe lo dice en su informe.
- [ ] La prueba de la tarea 1 pasa sobre todo lo nuevo.

### Tarea 3 — Guías por tarea
**Ficheros:** `docs/user/{en,es}/guides/*.md` (modelos locales, agente de perfil `.md`, agentes remotos A2A, vigilar un job,
compresión de salida, medir con `RUST_LOG`, atajos y paneles). Cada guía: objetivo en una línea, pasos numerados,
resultado esperado, qué hacer si no sale.
- Reutiliza los hechos de `docs/modelos-locales.md`, `docs/odd.md`, `docs/output-filter.md` y `docs/logging.md`
  **resumiéndolos y enlazando**, sin copiarlos.

### Tarea 4 — Referencia con sincronía automática
**Ficheros:** `docs/user/referencia/{comandos,atajos,configuracion,entorno,perfil}.md`, y la prueba de sincronía en
`crates/loomcoding-cli/tests/docs_usuario.rs`.
- `comandos.md` lista cada entrada de `COMMANDS` con su descripción y ejemplo. `configuracion.md` recorre todas las
  secciones de `loomcoding.toml` (`[provider]`, `[agents]`, `[compaction]`, `[limits]`, `[ui]`, `[a2a]`, `[flows.*]`,
  `[output_filter]`…) con tipo, valor por defecto y si requiere reiniciar.
- [ ] Pruebas: falla si un comando de `COMMANDS` no aparece en `comandos.md`; falla si un campo de las estructuras de
  configuración no aparece en `configuracion.md`; falla si la referencia cita algo que ya no existe.

### Tarea 5 — Imágenes y solución de problemas
**Ficheros:** `scripts/capturas/` (generador de imágenes a partir de los volcados del e2e; el formato se decide con un
experimento corto: SVG a partir del texto del volcado frente a PNG), `docs/user/img/`, `docs/user/problemas/*.md`.
- [ ] Prueba: cada imagen de `img/` tiene su origen declarado y se regenera igual (determinista); la guardia de la
  tarea 1 exige texto alternativo.
- Capturas mínimas: pantalla principal, cabecera de flock con ODD, panel `/settings`, aviso `job:`, cabecera
  `[filtrado: …]`. Todas con perfil temporal y sin datos personales.

## Entrega 2 — Ayuda dentro de la app

### Tarea 6 — `/help` por temas y por comando
**Ficheros:** `crates/loomcoding-tui/src/commands.rs` (descripciones y ejemplos por comando), `app/` (estado del
panel de ayuda), `render/overlays.rs` (`draw_help_overlay`), y un módulo nuevo de temas de ayuda alimentado por la misma
fuente que la referencia.
- `/help` abre el índice de temas; `/help <comando>` muestra su ayuda corta con ejemplo y un enlace a su página; los
  atajos siguen disponibles. La ayuda cabe en 40 columnas, sin parpadeo ni retraso al escribir (la composición del prompt
  no puede notar el panel).
- [ ] Pruebas: cada comando tiene ayuda; `/help <comando desconocido>` sugiere el más parecido; el panel respeta los
  anchos mínimos; volcado e2e en PTY con `LOOMCODING_HOME` temporal (`docs/mockups/ayuda-e2e.md`).

## Entrega 3 — Sitio web

### Tarea 7 — Sitio de documentación
Sitio propio en `loomcoding.com` (decidido). **Ficheros:** `site/` (Docusaurus con i18n: inglés por defecto y español;
el contenido es `docs/user/{en,es}/` tal cual), `CNAME`/configuración del dominio y un flujo de publicación (propuesta:
GitHub Pages; registro y DNS los hace la persona usuaria), flujo de construcción y publicación documentado, buscador, navegación
generada de la estructura de carpetas.
- [ ] Pruebas: la construcción falla con un enlace roto; la navegación contiene todas las páginas; las imágenes cargan.
- Documentar en `docs/user/README.md` cómo se construye y cómo se publica.

## Revisión (casos que ninguna tarea cubre sola)

- Una página nueva sin entrada en el índice → la guardia de la tarea 1 lo detecta.
- Un ajuste con nombre cambiado en el código → la sincronía de la tarea 4 falla en `cargo test`.
- Una persona nueva lee solo `empezar/` y llega a un primer prompt funcionando sin abrir nada más: se prueba a mano al
  cerrar la entrega 1 (con un perfil temporal vacío).
