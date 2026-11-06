# Aspecto unificado: paleta, estilo y distribución — spec

Estado: **propuesta**, pendiente de decisión. Nace de una observación del usuario: «se está haciendo
un popurrí de opciones: settings, themes, layout, temas».

## Problema

Lo que decide cómo se ve Loomcoding está repartido en conceptos que se solapan:

| Mando | Dónde se toca | Qué decide hoy |
|---|---|---|
| Tema (`ThemePreset`: Cloth, Martty, Mindfork, Pawan, Crush, MastraCode, Loomcoding) | `/theme` (cicla), `ui.toml` (`theme`) | La paleta y, solo con `Crush`, el `Look` entero |
| `Look` (`composer`, `tool`, `layout`, `chrome`) | Sin mando propio: lo trae el preset | Cuatro ejes de presentación en un solo paquete |
| Layout (`workspace`, `rail`, `minimal`, `cards`) | `/layout`, `ui.toml` (`layout`), `App::layout_override` | La distribución; **duplica** `Look::layout` |
| Muñeco, lluvia, ratón, menú | `/mascot`, `/sidebar`, `ui.toml` (`mascot`, `matrix_*`, `mouse`) | Interruptores sueltos |

Defectos concretos:

1. El layout vive dos veces (`Look::layout` y `layout_override`); `set_prefs_path` tiene que reconciliarlos a mano.
2. `chrome` (con líneas o sin ellas) y `composer`/`tool` no se pueden elegir por separado: solo cambian al
   cambiar de tema, y cambiar de tema también cambia los colores.
3. Cada widget nuevo (la barra de desplazamiento, el logo con efectos) tienta a añadir otro mando.
4. La ayuda no dice qué cambia cada mando.

## Intención

Tres capas, cada una con **un** sitio donde elegirla:

- **Paleta**: solo colores. `/theme`.
- **Estilo** (el `Look` sin el layout): `composer`, `tool`, `chrome`. `/style` con dos valores de partida,
  `classic` (con líneas) y `flat` (sin líneas); cada tema declara el suyo por defecto.
- **Distribución**: `/layout`.

Regla de diseño que ya se aplica a la barra de desplazamiento: **un widget nuevo deriva de la capa que ya
existe** (`chrome`, `layout`), no añade un mando propio.

## Objetivos y no objetivos

Objetivos: una sola fuente de verdad por capa; `ui.toml` con tres claves de aspecto (`theme`, `style`,
`layout`); compatibilidad con los ficheros `ui.toml` actuales; una tabla en la ayuda.

No objetivos: nuevos temas, nuevos estilos de widget, un editor visual de ajustes. Los interruptores de
funciones (muñeco, lluvia, ratón) se quedan como están: no son aspecto, son funciones.

## Preguntas resueltas

Resueltas en `2026-11-06-loomcoding-panel-ui.md`: `/style` es una fila del panel `/ui`; `composer`, `tool` y
`chrome` forman un solo estilo; y los temas de usuario pueden declarar `style` y `layout` como valor por
defecto.

## Fuera de esta spec

El plan de implementación se escribe cuando estas preguntas estén resueltas. Hasta entonces, la regla de
«derivar de la capa existente» es la única restricción vigente.
