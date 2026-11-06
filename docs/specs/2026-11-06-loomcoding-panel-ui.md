# Panel `/ui`: toda la capa visual bajo un solo comando — spec

Estado: **propuesta**. Continúa `2026-11-06-loomcoding-aspecto-unificado.md` y resuelve sus preguntas
abiertas. Nace de dos observaciones del usuario: el menú de `/` está lleno de comandos visuales
(«deberíamos dejar el principal para otras cosas») y conviene «unificarlos bajo uno, como config ui».

## Problema

El popup de `/` mezcla lo que se hace con lo que se ve. De 26 comandos, **ocho** son solo de aspecto o de
interfaz: `/theme`, `/layout`, `/temas`, `/crush`, `/classic`, `/mascot`, `/sidebar`, `/mouse`. Se solapan
(`/crush` y `/classic` son un tema con otro nombre; `/temas` ya abre un árbol que incluye el tema) y
ocultan los comandos de trabajo (`/agent`, `/queue`, `/compact`…).

## Intención

Un único comando, **`/ui`**, que abre el panel de apariencia. El menú principal queda para lo que se hace
con el agente y la sesión.

### Panel

Se **amplía `/temas`** (que ya es un árbol de overlay con `TemasLevel`) en lugar de crear otro: `/temas`
pasa a ser `/ui`. Raíz, en este orden y con el valor vigente a la derecha:

| Fila | Qué decide | Valores |
|---|---|---|
| Tema | Paleta (solo colores) | los `ThemePreset` |
| Estilo | `composer`, `tool` y `chrome` | `classic` (con líneas) · `flat` (sin líneas, el de `crush`) |
| Distribución | Layout | `rail` · `workspace` · `minimal` · `cards` |
| Efectos | Lluvia Matrix y efectos del logo | submenú existente, más `logo_fx` |
| Spinner | Indicador de actividad | lista existente |
| Muñeco | Mascota de estado | on · off |
| Menú lateral | Barra izquierda | on · off |
| Ratón | Captura del ratón | on · off |

`←`/`→` cambian el valor de la fila sin entrar; `Enter` entra en los submenús; `Esc` sube un nivel. Cada
cambio se aplica al momento y se guarda en `ui.toml`, como hoy. Con `NO_COLOR` el panel se abre igual.

### Línea de comandos

`/ui` abre el panel. `/ui <fila> [valor]` actúa sin abrirlo, para quien prefiere teclear:
`/ui layout rail`, `/ui style flat`, `/ui theme crush`, `/ui mascot off`. Sin valor, la fila **rota** al
siguiente, como hacían `/layout` y `/theme`. Un valor desconocido responde con los valores posibles.

### Alias

`/theme`, `/layout`, `/temas`, `/crush`, `/classic`, `/mascot`, `/sidebar` y `/mouse` **siguen funcionando**
(no se rompe la memoria muscular ni los scripts) pero **salen de la lista del popup**: se marcan como alias
ocultos y se resuelven como `/ui <fila>`. Si alguien teclea `/crush` exacto, se ejecuta; la ayuda de `F1`
los menciona como alias de `/ui`.

### Menú principal resultante

`/help`, `/new`, `/sessions`, `/settings`, `/queue`, `/tools`, `/cancel`, `/pause`, `/resume`, `/parent`,
`/agent`, `/focus`, `/agents`, `/a2a`, `/refresh-agents`, `/compact`, `/verbose`, `/ui`, `/quit`. Diecinueve
en vez de veintiséis, y el aspecto en una sola línea.

## Decisiones tomadas (resuelven la spec del aspecto unificado)

- `/style` **no** es un comando aparte: es la fila «Estilo» del panel. Resuelve la primera pregunta abierta.
- `composer`, `tool` y `chrome` forman **un solo** estilo (`classic`/`flat`); no se combinan a mano. Resuelve
  la segunda. Internamente el `Look` conserva sus ejes; el panel solo expone los dos paquetes.
- `layout_override` desaparece: el layout vive en el `Look` y `ui.toml` lo sobrescribe al cargar.
- `/verbose` y `/mouse` no son aspecto estrictamente; `/mouse` entra en el panel (es interfaz) y `/verbose`
  se queda fuera (es diagnóstico del proveedor).

## No objetivos

Temas nuevos, editor de colores, y cambiar la semántica de ninguna preferencia guardada: las claves de
`ui.toml` actuales siguen leyéndose igual.

## Preguntas abiertas

- ¿Los temas de usuario (`ThemeFile`) pueden declarar `style` y `layout`? (Propuesta: sí, como valores por
  defecto que `ui.toml` sobrescribe.)
- ¿Se muestra el panel como overlay central, como `/settings`, o como lista lateral? (Propuesta: overlay
  central, igual que `/temas` hoy.)
