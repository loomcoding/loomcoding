# Personalizar la interfaz con /ui, /theme y /layout

**Qué es**

Estos comandos cambian el aspecto de la pantalla: el tema de color, el estilo del transcript, la disposición y algunos interruptores. Un cambio se aplica al momento y se guarda en `ui.toml`, dentro de la carpeta de tu perfil.

**Para qué**

- Elegir un tema de color o un estilo del transcript.
- Elegir una disposición de pantalla, o mostrar la barra lateral.
- Activar o desactivar el muñeco de estado o la captura del ratón.

**Cómo**

1. Escribe `/ui` para abrir el panel de interfaz. `/temas` abre el mismo panel.
2. O fija una fila directamente, por ejemplo `/ui layout minimal`.
3. O cambia rápidamente con `/theme` (tema de color) y `/layout` (disposición).

## Ejemplo

```text
> /ui layout minimal
> /ui mouse off
> /theme
```

## Filas del panel

| Fila | Valores | Valor por defecto | Requiere reiniciar |
|---|---|---|---|
| `theme` | `cloth`, `martty`, `mindfork`, `pawan`, `crush`, `mastracode`, `loomcoding` | `cloth` | No |
| `style` | `classic`, `flat` | Lo fija el tema, sin reemplazo | No |
| `layout` | `workspace`, `rail`, `minimal`, `cards` | Lo fija el tema, sin reemplazo | No |
| `carril` (rail) | `tree`, `dots`, `bar`, `box`, `tint`, `none` | `tree` | No |
| `mascot` | `on`, `off` | `on` | No |
| `sidebar` | `on`, `off` | `off` | No |
| `mouse` | `on`, `off` | `on` | No |

La fila `effects` abre un submenú, que también abre `/ui effects`. La fila `spinner` solo se cambia en el panel, con `/ui` o `/temas`.

## Comandos

| Comando | Qué hace | Requiere reiniciar |
|---|---|---|
| `/ui [fila] [valor]` | Abre el panel, o fija una fila. | No |
| `/temas` | Abre el panel de interfaz. | No |
| `/theme` | Cambia de tema de color en orden. | No |
| `/layout` | Cambia la disposición en orden: `rail`, `minimal` y `cards`. | No |
| `/crush` | Cambia al estilo Crush. | No |
| `/classic` | Vuelve al estilo Cloth. | No |
| `/mouse` | Activa o desactiva la captura del ratón. Desactivada, puedes seleccionar texto con el ratón. | No |

El fichero es `<perfil>/.loomcoding/ui.toml`. `<perfil>` es tu carpeta personal, o la carpeta de `LOOMCODING_HOME` cuando esa variable está definida.

## Resultado

El panel muestra cada fila con su valor actual, y un cambio se ve al momento.

## Si no funciona

| Síntoma | Solución |
|---|---|
| Aparece «Unknown row» o «Unknown value». | El mensaje lista los nombres válidos. No cambia nada, y tu texto se queda en el campo de escritura. |
| Aparece un mensaje de «not saved». | El cambio se aplica a esta sesión, pero no se pudo escribir `ui.toml`. Revisa los permisos de la carpeta. |
| Los ajustes vuelven tras reiniciar. | El fichero no se escribió, o `LOOMCODING_HOME` apunta a otra carpeta. |

## Siguientes pasos

- [Qué pasa en el primer arranque](../start/first-run.md)
- [Enviar tu primer prompt](../start/first-prompt.md)
