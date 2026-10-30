# Especificación: composer de chat multilínea

Estado: implementada; validación de pegado en terminal Windows pendiente.

## Objetivo

Permitir redactar prompts de varias líneas sin que el cuadro de entrada crezca
indefinidamente ni que un salto de línea pegado envíe el prompt.

## Comportamiento

- El composer empieza con una línea visible y crece con el contenido hasta un
  máximo de tres filas visuales; al llegar al borde, el texto se ajusta a la
  siguiente fila automáticamente.
- Al superar esas tres líneas, el área mantiene su tamaño y desplaza el texto
  para mantener visible el cursor; todo el contenido sigue dentro del prompt.
- `Enter` envía explícitamente. `Ctrl+J` inserta una línea mientras se redacta.
- Pegar texto con uno o más `\n` inserta esas líneas en el composer y no envía
  el prompt. Enter sin modificadores envía después el bloque completo.
- El contenido y el desplazamiento deben funcionar con los looks Classic y
  Crush, respetando el tamaño disponible del terminal.

## Fuera de alcance

- Cambiar la tecla de envío o introducir nuevos atajos.
- Cambiar límites de longitud, historial, selección o formato del prompt.

## Aceptación

1. El área visible pasa de una a dos y tres líneas mientras se añade contenido.
2. Las líneas adicionales no aumentan el alto del cuadro y el cursor sigue
   visible desplazando el texto dentro del composer.
3. Pegar `uno\ndos\ntres\n` conserva exactamente el texto en el composer y no
   devuelve un prompt para enviar.
4. `Enter` envía el prompt completo; `Ctrl+J` añade una línea según la ayuda.
5. Las pruebas cubren Classic y Crush, así como límites de altura del terminal.
