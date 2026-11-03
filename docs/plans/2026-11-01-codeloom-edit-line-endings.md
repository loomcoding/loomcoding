# Plan: edición exacta con finales de línea de Windows

Spec: [`docs/specs/2026-11-01-codeloom-edit-line-endings.md`](../specs/2026-11-01-codeloom-edit-line-endings.md)

1. Añadir un helper pequeño que adapte saltos LF/CRLF al formato del archivo.
2. Usarlo en `edit` y `multiedit` para patrones y reemplazos.
3. Verificar formato, diff y compilación del core.
