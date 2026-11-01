# Estudio: plugins de ECC, ponytail y superpowers, y su encaje en CodeLoom

> Fecha: 2026-11-01. Estado: **estudio**, sin decisiones tomadas ni código. Los datos de las tres columnas salen de leer
> los ficheros locales de cada plugin (rutas al final); lo que es propuesta va rotulado como tal.
>
> Relacionado: `specs/2026-10-14-loom-skills-ponytail-superpowers.md` (arbitraje entre las dos familias de skills),
> `specs/2026-10-31-codeloom-ecc-agent-workforce-research.md` (investigación de ECC, de la persona usuaria, sin commitear),
> `specs/2026-11-01-codeloom-external-catalog-profiles.md` (catálogo externo, ya implementado).

## 1. Qué es un «plugin» en estos tres casos

No es un formato único ni un concepto de A2A: es una **carpeta con un manifiesto y varios tipos de componente**, definida por
Claude Code y copiada por otros arneses. Lo que CodeLoom llama «A2A» solo aparece, en estos plugins, en las tarjetas de agente
que algún perfil declare; el plugin como unidad no existe en A2A y no debe presentarse como tal.

| | superpowers 7.0.0 | ponytail 5.3.3 | ECC 2.2.3 |
|---|---|---|---|
| Manifiesto (`.claude-plugin/plugin.json`) | mínimo: nombre, versión, autor, licencia; sin listas de componentes (se infieren por carpetas) | declara `hooks` (2 ficheros), tipos y `userConfig` | declara `skills` y `commands` (arrays, validador estricto) y `userConfig` |
| Skills | 15 (`brainstorming`, `writing-plans`, `systematic-debugging`…) | 6 (`ponytail`, `-audit`, `-debt`, `-gain`, `-help`, `-review`) | 314 |
| Agentes | no | no | 71 |
| Comandos | no | no (se invocan como skills con `/ponytail lite\|full\|ultra`) | 98 (muchos son atajos de compatibilidad) |
| Hooks | **1** (`SessionStart`) | **3** (`SessionStart`, `SubagentStart`, `UserPromptSubmit`) | **28** en 8 eventos: `PreToolUse` 10, `Stop` 8, `SessionStart` 2, `PostToolUse` 2, `PostToolUseFailure` 2, `UserPromptSubmit` 1, `PreCompact` 1, `SessionEnd` 1 |
| Reglas / MCP / otros | no | tablero de coste (CSV, *webhook* opcional) | reglas por lenguaje, configuraciones MCP, perfiles y módulos de instalación selectiva |
| Configuración tipada (`userConfig`) | no | sí (incluye un token marcado como sensible, que guarda en el llavero del sistema) | sí (`hooks_enabled`, `hook_profile`) |
| Otros arneses | carpetas para Codex, Cursor, OpenCode, Kimi, Pi, Hermes, Gemini… | las mismas más Windsurf, Kiro, Cline, Qoder… | Codex, Pi, scripts de conversión de agentes |

Observación que importa: **los tres convergen en `.agents/`** (carpeta neutral) además de la propia de cada arnés. CodeLoom ya lee
`.agents/agents` y `.agents/skills`, así que el punto de contacto más barato ya existe.

## 2. Qué hace cada hook (lo medido)

- **superpowers:** el único hook, en `SessionStart` (`startup|clear|compact`), lee `skills/using-superpowers/SKILL.md`
  (3.192 bytes) y lo mete como contexto oculto. Es **inyección de contexto**, no ejecuta nada del proyecto.
- **ponytail:** en `SessionStart` escribe un fichero de bandera, emite su juego de reglas como contexto y, si falta, sugiere
  configurar la línea de estado; en `SubagentStart` repite la inyección para el hijo; en `UserPromptSubmit` un seguidor de modo
  mira si el usuario escribe `/ponytail …` y guarda el nivel (`lite|full|ultra|off`) en un fichero de bandera. Es **modo de sesión** persistido en un fichero, más contexto inyectado.
- **ECC:** 28 hooks de más variedad: `PreToolUse` sobre `Bash`, `Write`, `Edit`, `mcp__*`… (barandillas que pueden bloquear o
  modificar), `Stop` (cierre de turno: persistencia, notificación, calidad), `PostToolUse*` (registro), `PreCompact` y
  `SessionEnd`. Cada comando es un `node -e` que localiza la raíz del plugin. Aquí sí hay **ejecución de comandos de terceros**
  con efecto real. Falta clasificar cada uno por efecto (bloquea, informa, persiste); eso es objeto de la investigación de ECC,
  no de este estudio.

Resumen: de 32 hooks, **ninguno de superpowers ni de ponytail ejecuta lógica de proyecto**; ECC sí.

## 3. Qué tiene hoy CodeLoom (verificado en el código)

| Componente de plugin | En CodeLoom hoy |
|---|---|
| Skills (`SKILL.md`) | Sí, pero **solo las que declara un perfil de agente** (`scope.skills`), con topes de tamaño; de `.agents/skills` del proyecto y del usuario, y del catálogo externo. La sesión raíz **no tiene skills siempre activas** |
| Agentes | Sí: registro con perfiles globales, de proyecto y de catálogo, aprobación por huella |
| Comandos del usuario (`/nombre`) | Solo los propios de la TUI; no hay comandos importables |
| Hooks | **No existen.** El diseño base los nombra como posteriores («hooks pre/post-tool-use como middleware») |
| MCP | Sí |
| Configuración tipada con secretos en llavero | No |
| Instalación selectiva (módulos, perfiles) | Fuera de CodeLoom: es el catálogo del usuario |

## 4. Encaje por componente (propuesta)

**Skills.** Encajan por la vía que ya existe. Lo que **falta** es lo que usan superpowers y ponytail: skills **siempre activas en
la sesión raíz**, con un **nivel** (ponytail `lite|full|ultra`) y un **arbitraje** entre ellas (la spec de 2026-10-14 propone: ponytail
gobierna el tamaño de la solución; superpowers, el proceso; en tarea arquitectónica gana superpowers). Hoy eso no se puede
expresar porque la sesión raíz no carga skills.

**Agentes.** Resuelto con el catálogo externo; ECC entra por ahí.

**Hooks: tres capas, de menos a más riesgo.**

1. *Intención declarativa, sin ejecutar nada.* Dos de los tres plugins (superpowers y ponytail) usan los hooks para una sola cosa:
   inyectar texto en el prompt al empezar la sesión o al arrancar un hijo. CodeLoom puede ofrecer eso como **paquetes de contexto
   acotados** (un texto con tope, activado por evento, visible en el desglose de contexto), sin shell. Cubre los 3 hooks de
   superpowers y ponytail que inyectan contexto (`SessionStart` de ambos y `SubagentStart` de ponytail), con los topes del decálogo.
2. *Eventos declarativos de modo.* El cambio de nivel de ponytail es estado de sesión (un nivel y su texto), no un programa. Se
   modela como **modo de sesión** con valores cerrados.
3. *Comandos de shell de terceros* (la mayoría de los 28 de ECC). Es la parte peligrosa: ejecuta código ajeno con acceso al
   proyecto. Si algún día se admite, solo con aprobación por huella igual que los perfiles, sin heredar permisos, con tiempo
   máximo, salida acotada y política de fallo explícita (abierta o cerrada). **No recomiendo hacerlo ahora**: los permisos de
   herramienta y la aprobación por código que ya existen cubren buena parte de lo que ECC hace con `PreToolUse`, y de forma
   verificable en vez de a base de scripts.

**Comandos.** Un comando de plugin es una plantilla de prompt. Equivale a una skill invocable por el usuario; no hace falta un
concepto aparte, y de hecho ponytail ya los publica como skills.

**`userConfig`.** Los ajustes simples (booleanos, cadenas) caben en `codeloom.toml`. Los secretos en llavero no existen en
CodeLoom y no se proponen aquí.

**Instalación de plugins como tal (leer `plugin.json`, marketplaces).** No recomendado: acopla CodeLoom a un validador ajeno y
«estricto y no documentado» (las notas de ECC lo admiten). Mejor que **el catálogo del usuario exporte** cada plugin al formato
`.agents/` y deje constancia de lo que *no* importa (los hooks de shell), como ya hace con ECC.

## 5. Dirección elegida por la persona usuaria (2026-11-01) y orden propuesto

La conversación principal debe quedar **limpia**: no se le inyectan skills ni paquetes de contexto. Cada especialidad vive en un
**agente** (`superpowers`, `ponytail`…) que la persona usuaria invoca, con las skills que interesen en ese momento, en su propia
sesión y su propio contexto, visible dentro de la conversación principal como un nodo más. Esto sustituye a los pasos que este
estudio proponía antes (skills de sesión y paquetes de contexto por evento), que sí ensuciaban el prompt principal, y hace
innecesario el arbitraje entre superpowers y ponytail: nunca comparten prompt.

Lo que ya cubre CodeLoom: perfil con `scope.skills` cargadas solo en ese agente, sesión de hijo con su contexto, nodo en la
conversación con el estado A2A exacto, `input-required` del hijo hacia la persona usuaria, delegación recursiva (superpowers
reparte trabajo; necesita `max_depth` ≥ 2 en la configuración y en su perfil) y catálogo con aprobación por huella.

| Paso | Dónde | Qué | Riesgo |
|---|---|---|---|
| A | catálogo del usuario | exportar `superpowers` y `ponytail` como **agentes** con sus skills declaradas en `scope.skills`, anotando los hooks no importados | ninguno en CodeLoom |
| B | CodeLoom, spec propia | **invocación directa por la persona usuaria** (`/agent <nombre> <tarea>` y `@nombre`), sin pasar por el modelo principal. **Hecho:** `specs/2026-11-01-codeloom-user-invoked-agents.md` | bajo |
| C | CodeLoom, spec propia | **elegir skills al invocar** (`--skills a,b`), como subconjunto de las que el perfil declara (así la aprobación por huella las sigue cubriendo). **Hecho**, misma spec | bajo |
| D | solo si hace falta | hooks de shell de terceros, tras aprobación, con sandbox | alto; sin decidir |

## 6. Lo que este estudio no resuelve

- No mide cuánto ahorran o cuestan estas skills en CodeLoom: medirlo exigiría uso real de un modelo y su autorización.
- No clasifica los 28 hooks de ECC por efecto.
- No decide el arbitraje definitivo de superpowers y ponytail; solo remite a la propuesta de 2026-10-14.
- Las skills de ponytail y superpowers son texto que guía al modelo, no un control mecánico: el cumplimiento depende del modelo.
  Las garantías siguen estando en permisos, aprobación por código y topes.

## 7. Preguntas abiertas

1. Invocación directa: ¿`/agent <nombre> <tarea>`, `@nombre` en el compositor, o ambas? El compositor no debe sufrir (que no
   parpadee ni se retrase al escribir).
2. Elegir skills al invocar: ¿lista explícita (`--skills brainstorming,writing-plans`) o selección en una ventana?
3. ¿El resultado del agente vuelve a la conversación principal solo como resumen acotado (como hoy el artefacto del hijo), o la
   persona usuaria puede abrir su sesión completa? Hoy ya puede entrar en el hijo desde el carril.
4. ¿Un agente puede declarar skills que *no* se cargan por defecto y solo se activan al pedirlas? Propuesta: sí, es el caso de
   superpowers con sus 15 skills.

## Fuentes locales leídas

- superpowers: `~/.claude/plugins/cache/claude-plugins-official/superpowers/7.0.0/` (`.claude-plugin/plugin.json`,
  `hooks/hooks.json`, `hooks/session-start`, `skills/`).
- ponytail: `G:\loom-references\ponytail\` (`.claude-plugin/plugin.json`, `hooks/claude-codex-hooks.json`,
  `hooks/ponytail-activate.js`, `skills/`).
- ECC: `E:\workspace\personal\ECC\` (`.claude-plugin/plugin.json`, `.claude-plugin/PLUGIN_SCHEMA_NOTES.md`, `hooks/hooks.json`,
  `agents/`, `skills/`, `commands/`).
