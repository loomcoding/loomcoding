# Ratón y scroll del composer

## Objetivo

Hacer visible y utilizable el scroll vertical del composer en textos largos, y
arrancar CodeLoom con captura del ratón activa.

## Alcance

- El valor inicial y la preferencia actual de la TUI son `mouse = true`; `/mouse`
  aún permite desactivarlo y guarda esa decisión.
- El composer muestra un scrollbar vertical de una columna cuando su contenido
  desborda la altura visible.
- La rueda sobre el composer mueve el cursor por sus líneas y actualiza su
  ventana visible; la rueda en el resto de la conversación sigue desplazando el
  transcript.
- El thumb acompaña el desplazamiento y aparece al inicio y al final según la
  posición real del composer.

## Fuera de alcance

- Cambiar el modelo de navegación o las teclas de edición.
- Añadir dependencias, un estado de scroll paralelo o nuevas preferencias.
- Cambiar el scroll del transcript o permitir arrastrar el thumb.

## Aceptación

1. Una preferencia ausente arranca con captura del ratón activada; la preferencia
   guardada actual también queda activada.
2. La rueda sobre el composer desplaza filas visibles (incluido texto que se
   ajusta en Crush) sin cambiar el viewport del transcript; se puede bajar desde
   la mitad hasta el final.
3. La barra es de una columna, solo aparece con overflow y su thumb se mueve.
4. Las pruebas cubren el valor inicial, el scroll por rueda y el render/posición
   del thumb.
