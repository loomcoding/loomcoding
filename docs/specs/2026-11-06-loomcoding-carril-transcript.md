# Carril del transcript configurable — spec

Estado: **propuesta**. Nace de una petición del usuario: las líneas que acompañan a su pregunta y a la respuesta
del modelo en el transcript podrían tener otro estilo (puntos, otro trazo) o suprimirse, «como hace Codex o
Claude Code», mirando también cómo lo resuelven Crush, MastraCode o Pi.

## Situación actual

Cada bloque del transcript (mensaje, herramienta, subagente) se pinta con un **carril de árbol** de 3 columnas a
la izquierda del texto (`render/transcript.rs`, `draw_tree_gutter`): `╭─` en la primera fila, `│` en el cuerpo y
`╰─` en la última (o `──` si ocupa una sola). La cabecera lleva el glifo del rol (`»` usuario, `≋` asistente,
`⋯` razonamiento…). El bloque con foco usa el trazo grueso `┏━ ┃ ┗━`. El carril se omite por debajo de 40
columnas de ancho.

## Lo que hacen las referencias

| Referencia | Cómo separa los mensajes |
|---|---|
| Codex | sin carril: el mensaje del usuario lleva un fondo tenue y el del asistente un prefijo `• ` atenuado |
| Crush | sin carril: solo el mensaje **enfocado** lleva un `▌` de media celda en el margen izquierdo |
| MastraCode | solo el mensaje del **usuario** va en una caja redonda fina (`╭──╮ │ » texto │ ╰──╯`) del ancho de su contenido, en verde, con el prompt `»`; el del asistente va sin caja ni trazo |
| Pi | sin trazo: el mensaje del usuario lleva un **fondo de color** (`userMessageBg`) y el del asistente va como texto plano con sangría |

## Intención

Una fila nueva del panel `/ui`, «Carril», con seis valores. Es una preferencia de aspecto más, por tanto
también se fija con `/ui carril <valor>` y se guarda en `ui.toml` (clave `rail`).

| Valor | Aspecto |
|---|---|
| `tree` | el actual (`╭─ │ ╰─`). Sigue siendo el valor por defecto mientras la persona usuaria no elija otro |
| `dots` | punteado: la cabecera lleva el glifo del rol y el cuerpo, `┆` en un gris tenue; sin esquinas |
| `bar` | estilo Crush: sin trazo, salvo un `▌` en el bloque enfocado; la cabecera conserva el glifo del rol |
| `none` | estilo Codex y Claude Code: sin trazo alguno; solo el glifo del rol al abrir el bloque, y el carril se reduce a 2 columnas |
| `box` | estilo MastraCode: solo el mensaje del usuario va en caja redonda fina del ancho de su contenido; el resto, sin trazo |
| `tint` | estilo Pi y Codex: sin trazo; el mensaje del usuario lleva un fondo tenue y el del asistente un `• ` atenuado |

## Reglas

- El bloque enfocado debe seguir viéndose entero en `tree` y `dots` (trazo grueso o `┇`); en `bar` y `none` basta
  el `▌` o el glifo con el color del rol.
- El cambio de valor no re-renderiza el texto: solo cambia lo que se parchea en las celdas del carril y su ancho
  (`tree_gutter`).
- El composer no puede parpadear ni ir con retraso: el cambio de ancho del carril se resuelve en el siguiente
  fotograma, sin ticks nuevos.
- Sin red, sin dependencias nuevas; ficheros nuevos ≤ 400 líneas.

## No objetivos

Cambiar los glifos de rol, el aspecto de las herramientas, o el ancho mínimo de 40 columnas. No se toca el
fondo tintado de Codex (queda como idea aparte).

## Preguntas abiertas

1. ¿Qué valor debe ser el predeterminado? Propuesta: `tree` hasta que la persona usuaria lo cambie.
