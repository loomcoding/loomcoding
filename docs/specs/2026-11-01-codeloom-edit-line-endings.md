# Especificación: edición exacta con finales de línea de Windows

## Objetivo

Permitir que `edit` y `multiedit` apliquen bloques multilínea copiados de la
salida de `view` en archivos CRLF, sin cambiar el formato de fin de línea del
archivo.

## Comportamiento

- Normalizar los saltos recibidos en `old` y `new` al formato de fin de línea
  del archivo antes de buscar y reemplazar.
- Mantener la validación actual de coincidencia única y la escritura existente.

## Aceptación

Un bloque multilínea con saltos LF puede reemplazar su equivalente CRLF; un
bloque inexistente o ambiguo sigue fallando sin escribir.
