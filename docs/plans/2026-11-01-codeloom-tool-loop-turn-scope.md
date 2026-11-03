# Plan: alcance de la detección de bucles de herramientas

Spec: [`docs/specs/2026-11-01-codeloom-tool-loop-turn-scope.md`](../specs/2026-11-01-codeloom-tool-loop-turn-scope.md)

1. Limitar las firmas comparadas por `should_stop` al historial desde el último
   mensaje de usuario.
2. Comprobar que el crate compila y que el diff queda limpio.

No cambiar el tope de pasos ni la ejecución y autorización de herramientas.
