# Dashboard limpio: tres diseños (mock)

Generado por `G:\temp\loom-qwen\mock-dashboard.py` con el mismo escenario. Con colores: `python mock-dashboard.py a|b|c|hoy|todos`.
Un solo acento (rosa del logo), glifos de estado y nada de cajas.

```text

=== HOY  (reconstruido del volcado e2e) ============================================================
 L∞MCODING ╱╱╱╱  ~/proyecto  • mock-model  • ready  • 0 subagents running · 0 jobs
   MCP not configured  ·  LSP rust-analyzer found  ·  skills 0/0 cards, 0 scopes
 ╭» You                                                           │CODELOOM
 ╰─ delega la inspeccion                                          │~/proyecto
 ─◌ sys: provider request started for `mock-model`                │
 ─◌ sys: provider response received for `mock-model`              │MODEL
 ╭─ ● subagent  inspector                                         │mock-model
 ╰─ │ {"child_id":2,"job_id":"subagent-1"}                        │responding · 0.0s
 ╭─ ▾ ✗ subagent  inspector [default] (failed) · step 3/3         │step 2/3
 │  ├─ ✓ ls                                                       │↑4353 ↓22
 │  └─ ✗ error: tool error: subagent reached its step limit       │
 ╰─ response                                                      │CAPABILITIES
 ─◌ sys: provider request started for `mock-model`                │MCP  not configured
 ╭≋ Assistant                                                     │LSP  rust-analyzer found · on …
 ╰─ delegado, espero al hijo                                      │SKILLS  0/0 cards · 0 scopes d…
                                                                  │TOOLS
                                                                  │● subagent
                                                                  │SUBAGENTS · 0 · 7.6k/42
                                                                  │✗ [default] inspector
                                                                  │  3/3 · 7.6k/42 · 0s
                                                                  │
                                                                  │
                                                                  │
                                                                  │
 > Describe what you want to build…                               │
enter send  ctrl+j newline  ctrl+k queue  ctrl+o fold  ctrl+t tools  ctrl+b sidebar  f2 settings  ctx 2.2k
```

```text

=== A  Claude/Codex: sin panel, estado en una linea ================================================
 loom · Ornith-1.5-9B · low                           ctx 12k/64k ██░░░░░░  gauntlet #3 · ronda 2/3

 › modo gauntlet: construye la orientación previa del plan

 ◌ gauntlet #3 · constructor trabajando · ronda 2/3

 ✓ view  docs/plans/…contexto-estructural.md
 ✓ write crates/loomcoding-core/src/config/orientation.rs
 ✓ bash  cargo test -p loomcoding-core  0.9s
 ✗ inspector  failed · 3/3 pasos · ↑7.6k ↓42
     └ subagent reached its step limit without a final response

 He creado `ContextConfig` aparte de `LoomcodingConfig`, tal como pide la Tarea 6.
 Falta enganchar `orientation_for` en `run_with_origin`.












 ❯ Describe lo que quieres construir…
 enter envía · ctrl+o pliega · ctrl+b panel · ? atajos
```

```text

=== B  Crush: panel estrecho, sin cajas ni separadores =============================================
 loom · Ornith-1.5-9B · low                                                    ctx 12k/64k ██░░░░░░

 › modo gauntlet: construye la orientación previa del plan         Agentes
                                                                   ◐ constructor   3m12s
 ◌ gauntlet #3 · constructor trabajando · ronda 2/3                ○ critico       cola
                                                                   ✓ explorer      40s
 ✓ view  docs/plans/…contexto-estructural.md                       ✗ inspector     12s
 ✓ write crates/loomcoding-core/src/config/orientation.rs
 ✓ bash  cargo test -p loomcoding-core  0.9s                       Tokens
 ✗ inspector  failed · 3/3 pasos · ↑7.6k ↓42                       entrada 12.3k  salida 1.1k
     └ subagent reached its step limit without a final response    caché   8.0k leídos

 He creado `ContextConfig` aparte de `LoomcodingConfig`, tal co…   MCP – · LSP rust-analyzer
 Falta enganchar `orientation_for` en `run_with_origin`.












 ❯ Describe lo que quieres construir…
 enter envía · ctrl+o pliega · ctrl+b panel · ? atajos
```

```text

=== C  Gentle: tarjeta de agentes sobre el compositor ==============================================
 loom · Ornith-1.5-9B · low                           ctx 12k/64k ██░░░░░░  gauntlet #3 · ronda 2/3

 › modo gauntlet: construye la orientación previa del plan

 ◌ gauntlet #3 · constructor trabajando · ronda 2/3

 ✓ view  docs/plans/…contexto-estructural.md
 ✓ write crates/loomcoding-core/src/config/orientation.rs
 ✓ bash  cargo test -p loomcoding-core  0.9s
 ✗ inspector  failed · 3/3 pasos · ↑7.6k ↓42
     └ subagent reached its step limit without a final response

 He creado `ContextConfig` aparte de `LoomcodingConfig`, tal como pide la Tarea 6.
 Falta enganchar `orientation_for` en `run_with_origin`.






 ❀ Agentes                                                                                 ▾ plegar
  ◐  constructor  orientation.rs                                           9B · low    4.2k   3m12s
  ○  critico      en cola                                                  9B · none      —       —
  ✓  explorer     mapa de config                                           9B · none   1.1k     40s
  ✗  inspector    sin respuesta final                                      9B · low    7.6k     12s

 ❯ Describe lo que quieres construir…
 enter envía · ctrl+o pliega · ctrl+b tarjeta · ? atajos
```
