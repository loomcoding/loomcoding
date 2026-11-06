# Volcados reales del carril del transcript

Capturados con `cargo test -p loomcoding-cli --test tui_pty_carril` (perfil aislado, proveedor `wiremock`, 40×120, disposición `minimal`). Cabe señalar que son pantallas reales de la TUI; se ha recortado a la zona del carril y se ha quitado la lluvia de caracteres del fondo, que es un efecto decorativo ajeno al carril.

## `tree`

```text
 ╭» 👤
 │  uno
 ╰─ dos
 ╭≋ 🤖
 ╰─ respuesta del carril
```

## `dots`

```text
 »  👤
 ┆  uno
 ┆  dos
 ≋  🤖
 ┆  respuesta del carril
```

## `bar`

```text
  » 👤
    uno
    dos
  ≋ 🤖
    respuesta del carril
```

## `box`

```text
 ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
 │ »uno
 │  dos
 ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
 ≋  🤖
    respuesta del carril
```

## `tint`

```text
 » 👤
   uno
   dos
 • 🤖
   respuesta del carril
```

## `none`

```text
 » 👤
   uno
   dos
 ≋ 🤖
   respuesta del carril
```

Nota: en 	int el mensaje del usuario lleva además un fondo tenue (el color de selección del tema), que un volcado de texto no muestra.
