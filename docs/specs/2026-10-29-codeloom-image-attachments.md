# Especificación: adjuntar imágenes desde el chat

Estado: implementado; el build y los tests del workspace pasan. Falta la revisión visual en la TUI real.

## Objetivo

Permitir adjuntar imágenes locales a un prompt de CodeLoom y enviarlas como
contenido multimodal a los proveedores OpenAI-compatible y Anthropic.

## Interacción

- Ctrl+V adjunta la imagen del portapapeles al borrador actual; el pegado de
  texto conserva su comportamiento habitual.
- Los adjuntos aparecen en una fila con fondo justo encima del composer y fuera
  del texto del prompt; se permiten hasta cuatro.
- Si el composer está vacío, Backspace quita el último adjunto.
- El siguiente Enter envía texto e imágenes en un único mensaje. La imagen se
  conserva en la sesión para los turnos siguientes.
- Las imágenes del portapapeles se normalizan a PNG y se validan antes de
  adjuntarlas.

## Límites y seguridad

- Cada imagen codificada debe medir como máximo 2 MiB.
- Las imágenes se leen al adjuntar, se mantienen en memoria y cruzan el IPC
  local como datos Base64. No se crean archivos temporales.
- El tamaño máximo del frame IPC admite hasta cuatro imágenes con ese límite.
- El nombre visible es solo el nombre base del archivo, nunca su ruta completa.
- Los modelos o endpoints que no acepten imágenes pueden rechazar el mensaje;
  CodeLoom muestra el error del proveedor.

## Alcance excluido

No se implementa previsualización gráfica, recorte ni otros tipos de archivo.

## Aceptación

1. Adjuntar una imagen válida presenta su nombre encima del composer y no inicia un
   turno por sí solo.
2. Un prompt posterior incluye las partes de texto e imagen, mantiene el origen
   humano y persiste ambas en la sesión.
3. Los adaptadores OpenAI-compatible y Anthropic emiten los bloques de imagen
   nativos adecuados.
4. Se rechazan formatos desconocidos, firmas incorrectas e imágenes superiores
   a 2 MiB sin perder los adjuntos ya aceptados.
5. La cola local conserva texto e imágenes al esperar su turno y al editar el
   texto de un prompt en cola.
