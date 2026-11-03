# Plan: identidad persistente de sesiones por proyecto

Spec: [`docs/specs/2026-11-01-codeloom-session-project-identity.md`](../specs/2026-11-01-codeloom-session-project-identity.md)

1. Usar una clave estable basada en el directorio canónico al crear el agente.
2. Mantener en SQLite la lectura y validación de sesiones con la clave anterior.
3. Comprobar formato, compilación y diff; no alterar registros de la base.
