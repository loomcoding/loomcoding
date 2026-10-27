# CodeLoom — Navegación de la documentación

> Estado: aprobado por la petición del usuario del 2026-10-26.
>
> Plan: `docs/plans/2026-10-26-codeloom-documentation-navigation.md`.

## Objetivo

Hacer más fácil encontrar y entender la documentación actual sin mover archivos
ni cambiar el contenido de los diseños históricos.

## Alcance

- Añadir `docs/README.md` como índice de entrada para la documentación.
- Agrupar los enlaces por función: arquitectura/diseño, planes, referencias y
  snapshots de estado.
- Reducir la lista duplicada del README raíz y enlazar el nuevo índice.
- Corregir en el README raíz el estado de capacidades que ya figuran como
  implementadas en el contexto vigente del proyecto.

No reorganizar físicamente `docs/specs/`, `docs/plans/`, `docs/reference/` ni
`docs/references/`: los enlaces actuales dependen de esas rutas.

## Criterios de aceptación

1. Desde el README raíz se llega a un índice claro de toda la documentación.
2. El índice explica qué contiene cada carpeta y enlaza los documentos de
   entrada y las propuestas activas más relevantes.
3. No se cambian de ruta ni se reescriben documentos ajenos a este alcance.
4. El README raíz no presenta OAuth2, mTLS ni la integración con herdr como
   capacidades pendientes.
