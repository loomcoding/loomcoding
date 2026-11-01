# Plan — Límites de tamaño para skills locales

> Estado: implementado (2026-11-01).
> Spec: `docs/specs/hechos/2026-10-31-codeloom-skill-size-policy.md`.

## Restricciones

- Las cuatro comprobaciones pasan. Sin red ni gasto; no se ejecuta contenido de ninguna skill.
- Se mantienen la carga explícita por `scope.skills`, la validación de IDs y raíces, la precedencia proyecto/usuario, el
  máximo de 16 skills y el límite de 2 KiB de frontmatter.

## Tareas

1. **Pruebas primero** en `skills.rs`: skills de 4 KiB a 24 KB y más de 500 líneas cargan; un fichero de más de 32 KiB es
   `Oversized`; el agregado nunca pasa de 8.000 tokens estimados (`PromptLimit` sin contenido parcial); el preámbulo cuenta
   contra el presupuesto; los tokens atribuidos cuadran con el prompt (salvo el redondeo por sección).
2. **Cargador:** `MAX_SKILL_FILE_BYTES` = 32 KiB y `MAX_SKILL_PROMPT_TOKENS` = 8.000 (en bytes, 32.000).
3. **Composición del hijo** (hueco no previsto en la spec): `compose_child_system` con cota propia para la persona y para
   las skills, y `MAX_SYSTEM_EXTRA_BYTES` del agente derivado de ambas más la holgura del marcador de recorte.
4. **Cierre:** cuatro comprobaciones y commit.
