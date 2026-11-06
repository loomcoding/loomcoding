# Pendientes sueltos — lista viva

Lo que no pertenece a un plan concreto. Se tacha al hacerse; las decisiones abiertas están marcadas.

## En marcha

- Menú izquierdo (`2026-11-06-loomcoding-menu-izquierdo.md`): T4 diff desplegable, T5 efectos del logo,
  T6 enganche y `/logofx`, T7 docs.

## Sueltas

- [ ] **Dashboard T9**: docs (`INDEX`, `CODE-MAP`, `ARCHITECTURE`, CHANGELOG) y volcados e2e de las tres
  disposiciones, con capturas en `docs/mockups/`.
- [x] **Integrar `dashboard-limpio` en master** (hecho el 6 de noviembre, avance rápido). Falta recompilar los binarios.
- [ ] **Cablear `ContextConfig::load` + `with_context_config`** en `loomcoding-cli`.
- [ ] **Contexto estructural, Tarea 7**: docs.
- [ ] **Modelo y esfuerzo reales por agente** en la tarjeta (enlazar el perfil al `SubagentDescriptor`; toca
  el core).
- [ ] **Título de la ayuda**: dice «Press F1 or Esc» aunque `?` también cierra.
- [ ] **F0/F1**: extraer los flujos del core con el 9B libre.

- [ ] **Enlazar los temas de usuario a la TUI**: hoy `Theme::from_toml` solo lo llaman las pruebas y ningún código de
  producción lee un `theme.toml` de usuario, así que `style` y `layout` de un tema de usuario (ya soportados en
  `ThemeFile`) no tienen efecto. Hay que decidir dónde vive el fichero y cargarlo en `App` antes de aplicar `ui.toml`.

- [ ] **Test intermitente** `loomcoding-a2a` `sync::tests::the_periodic_loop_refreshes_again_and_again`: falló una vez en la suite
  completa (sync.rs:439) y pasa 5 de 5 aislado; depende de la temporización del bucle periódico. Hay que hacerlo determinista.

## Decisiones abiertas

- ¿`/mascot on` debe forzar el braille aunque haya `NO_COLOR`?
- Ninguna más por ahora (las del panel `/ui` y el aspecto unificado las resolvió el usuario el 6 de noviembre).
