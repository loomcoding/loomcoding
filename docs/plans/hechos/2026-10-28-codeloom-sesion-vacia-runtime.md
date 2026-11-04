# Plan — Ownership de sesiones vacías en runtime

> Estado: **hecho** (2026-11-04). Implementado: `session_belongs_to_project` y su regresión; los cuatro checks pasan. La prueba manual de tool en la TUI con OpenRouter queda como opcional, sin bloquear nada.

Spec: `docs/specs/hechos/2026-10-28-codeloom-sesion-vacia-runtime.md`.

1. [x] Añadir al `SessionStore` una consulta de pertenencia por proyecto y cubrir SQLite con sesiones vacías y ajenas.
2. [x] Usar esa consulta en las dos validaciones del servidor y cargar directamente la sesión actual al conectar, sin cambiar el listado.
3. [x] Añadir regresión del runtime para cargar una sesión vacía y rechazar una ajena.
4. [x] Los cuatro checks requeridos pasan. La prueba normal se hizo con el daemon actualizado, pero el provider no respondió; queda documentado en el plan de PTC.
5. [x] (opcional, cerrado sin hacer) Repetir la prueba normal de tool en TUI cuando OpenRouter responda; solo entonces probar PTC integrado en runtime. El par live del adapter ya se repitió y volvió a ser rechazado; no afirma soporte ni ahorro.
