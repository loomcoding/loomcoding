# Volcados reales de /ui, /effort y /provider

Capturados con `cargo test -p loomcoding-cli --test tui_pty_comandos` (perfil aislado, proveedor `wiremock`, 40×130). Cabe señalar que son pantallas reales de la TUI, no bocetos.

## ui-panel

```text
 loom · mock-model · ready
                                                                                                    Session
                                                                                                    ready · 0.0s
                                                                                                    MCP not configured · LSP
                                                                                                    rust-analyzer found · skills
                                                                                                    0/0 cards, 0 scopes
                            ╭ Interfaz ─────────────────────────────�
                            │▸ Tema          ‹ Crush · Charmtone ›                                   │
                            │  Estilo        ‹ flat ›                                                │
                            │  Distribución  ‹ rail ›                                                │
                            │  Efectos       matrix off · logo on ›                                  │
                            │  Spinner       ‹ Aleatorio (uno distinto por petición) ›               │
                            │  Muñeco        ‹ on ›                                                  │
                            │  Menú lateral  ‹ on ›                                                  │
                            │  Ratón         ‹ on ›                                                  │
                            │                                                                        │
                            │                                                                        │
                            │                                                                        │
                            │                                                                        │
                            │                                                                        │
                            │                                                                        │
                            ╰─────────────────────────────────
 > Describe what you want to build…
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts
```

## effort-picker

```text
 loom · mock-model · ready
                                                                                                    Session
                                                                                                    ready · 0.0s
                                                                                                    MCP not configured · LSP
                                                                                                    rust-analyzer found · skills
                                                                                                    0/0 cards, 0 scopes
                                             ╭ Effort ────────────────────────�
                                             │› ● default                           │
                                             │    low                               │
                                             │    medium                            │
                                             │    high                              │
                                             ╰Enter apply · Esc cancel──────────────╯
 > Describe what you want to build…
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts
```

## provider-picker

```text
 loom · mock-model · ready
                                                                                                    Session
                                                                                                    ready · 0.0s
                                                                                                    MCP not configured · LSP
                                                                                                    rust-analyzer found · skills
                                                                                                    0/0 cards, 0 scopes
                             ╭ Provider ─────────────────────────────�
                             │Filter:                                                               │
                             │›   Uno · up (2)  uno · http://127.0.0.1:15672/v1                     │
                             │    Dos · sin acceso (clave)  dos · http://127.0.0.1:15680/v1         │
                             │    Tres · down  tres · http://127.0.0.1:15682/v1                     │
                             ╰Enter apply · Esc cancel────────────────────────�
 > Describe what you want to build…
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts
```

## provider-selected

```text
 loom · modelo-a · ready
                                                                                                    Session
                                                                                                    ready · 0.0s
                                                                                                    MCP not configured · LSP
                                                                                                    rust-analyzer found · skills
                                                                                                    0/0 cards, 0 scopes
 > Describe what you want to build…
Settings saved and applied
```
