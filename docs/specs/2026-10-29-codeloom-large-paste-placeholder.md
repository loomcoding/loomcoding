# Especificación: marcador para pegados largos

Estado: implementado; pendiente de verificación final y prueba visual en la TUI.

## Objetivo

Evitar que un pegado grande ocupe todo el composer mostrando un marcador breve,
sin perder ni alterar el contenido original que Loomcoding enviará al modelo.

## Comportamiento

- Solo se colapsan eventos de pegado cuyo texto exceda 1000 caracteres Unicode.
  El umbral es fijo y un pegado de 1000 caracteres permanece visible completo.
- El composer muestra un marcador como `[Pasted Content 3303 chars]`, con el
  recuento exacto de caracteres, incluidos los saltos de línea.
- El marcador representa un elemento atómico con identidad propia; el texto del
  usuario que casualmente coincida con el mismo marcador permanece literal.
- El contenido original completo se conserva en memoria en la posición donde se
  pegó. Al pulsar Enter, se expande antes del envío y el modelo recibe el texto
  exacto, en su orden original. Nunca se envía el marcador por sí solo.
- Backspace o Delete junto al marcador lo elimina como una sola unidad y también
  descarta su contenido asociado. El cursor y la edición de texto antes y después
  respetan ese límite. Para cambiar el contenido se elimina y se vuelve a pegar.
- Varios pegados largos en un mismo borrador permanecen independientes y se
  expanden en el orden de inserción. No se escriben a disco antes del envío.

## Límites

- El marcador reduce el texto visible en el composer; no reduce los tokens del
  mensaje que recibe el modelo, porque el contenido se conserva completo.
- No se añade configuración de umbral, previsualización ni almacenamiento de
  pegados entre sesiones.

## Aceptación

1. Pegados de hasta 1000 caracteres se insertan completos; uno de 1001 o más se
   representa con el marcador y el recuento correcto.
2. El recuento usa caracteres Unicode y no bytes UTF-8.
3. Enviar expande cada elemento en su lugar y conserva exactamente texto,
   saltos de línea y orden; no aparece el marcador en el mensaje enviado.
4. El texto literal `[Pasted Content 3303 chars]` escrito por el usuario nunca
   se sustituye por contenido pendiente.
5. Backspace/Delete elimina solo el elemento adyacente completo y no deja
   contenido huérfano ni marcadores duplicados.
6. Dos pegados grandes, incluso del mismo tamaño, se envían completos e
   independientes.

## Referencia de interfaz

Codex CLI usa como precedente un umbral de 1000 caracteres y marcadores de la
forma `[Pasted Content N chars]`; mantiene el contenido pegado pendiente para
expandirlo al preparar el envío. Se adopta ese patrón de interfaz, no su código
ni sus resultados. Fuente consultada el 2026-10-29:
[`chat_composer.rs`](https://github.com/openai/codex/blob/main/codex-rs/tui/src/bottom_pane/chat_composer.rs).
