# Compresión de salida y registro de 	racing — volcado E2E

Volcados reales de la prueba crates/loomcoding-cli/tests/tui_pty_compresion.rs, ejecutada en la TUI real (ConPTY) con
un proveedor simulado (mock_provider_compresion, sin red). El ash del guion imprime 40 líneas idénticas y una línea
final; el filtro general las colapsa a uido ×40 y deja la cabecera de filtrado. Guía de uso: docs/output-filter.md.

Rutas anonimizadas: el perfil temporal aparece como C:\work\. Ningún dato del usuario de la máquina queda en el volcado.

## 1. Turno con el filtro encendido ([output_filter] enabled = true, RUST_LOG=output_filter=info)

Se ve el ash, la cabecera [filtrado: …] (con la ruta del original), la salida colapsada uido ×40 y la respuesta final.

```text
 loom · mock-model · ready · ODD        < ﾔ  ﾖﾅﾘ         ｿﾛ               ﾒ             ｶ      ﾛ  ﾊ                *
 ╭» 👤                 ﾇﾎ                >    ﾗ ﾙ         ﾀ                ﾓ      ﾌ      ｷﾝSession                  +
 ╰─ ejecuta el ruido  ﾈﾏ                     ﾘ ﾚ         ﾁ                ﾔ      ﾍ      ｸ0responding · 0.0s        -
 ╭─ ● bash  (for /L %i in (1,1,40) do @echo ruido) & echo FIN             ﾕ      ﾎ      ｹ1step 2/3                ｵ<
 │  │ [filtrado: 39 líneas omitidas de 41 — regla general; el original sigue en  ﾏ ﾜ     2     1                  ｶ>
 │  C:\work\.loomcoding\tool-output\tool-call-ba  3Tokens                  ｷｱ
 │  sh-ruido-1794092810814588300-0.txt]     ｿ  0                             8   ﾑ 0     4↑7164 ↓30               ｸｲ
 │  │ ruido ×40                             ﾀ  1                             9   ﾒ 1     5     4                  ｹｳ   ﾎ
 ╰─   … (1 lines hidden) [Ctrl+O to expand] ﾁ  2                             :   ﾓ 2     6MCP not configured · LSPｺｴ   ﾏ
 ╭≋ 🤖         5                             ﾂ  3           ｶ                 .   ﾔ 3     7rust-analyzer found · skills ﾐ
 ╰─ Hecho: la salida ruidosa llegó filtrada al modelo.     ｷ                 =   ﾕ 4     80/0 cards, 0 scopes     ｼ    ﾑ
 ｶ  ｼ         7                            9ﾄ  5           ｸ                       5                              ｽ    ﾒ
 ｷ  ｽ         8                  =         :ﾅ  6       0   ｹ    ﾘ3                 6      Tools                   ｾ    ﾓ
 ｸ  ｾ         9                  *         .ﾆ          1   ｺ    ﾙ4                 7      ● bash                  ｿ    ﾔ
 ｹ  ｿ         :                  +         =ﾇ          2   ｻ    ﾚ5                 8                              ﾀ    ﾕ
 ｺ  ﾀ         .                  -         *           3   ｼ    ﾛ6                 9                              ﾁ    ﾖ
 ｻ  ﾁ    ｺ    =           ｿ      <         +           4   ｽ    ﾜ7                                                     ﾗ
 ｼ  ﾂ    ｻ                ﾀ      >         -           5   ｾ    ﾝ8                                                  ﾔ  ﾘ
 ｽ       ｼ                ﾁ      ｱ         <           6   ｿ    09                                                  ﾕ  ﾙ
 ｾ       ｽ                ﾂ      ｲ                     7   ﾀ     :         =                                        ﾖ
 ｿ       ｾ       ｲ        ﾃ      ｳ                     8   ﾁ     .   ﾎ     *                                        ﾗ
         ｿ       ｳ        ﾄ 4    ｴ                     9         =   ﾏ     +                                        ﾘ
         ﾀ       ｴ        ﾅ 5    ｵ                     :         *   ﾐ     -                                        ﾙ
         ﾁ       ｵ        ﾆ 6    ｶ                     .         +   ﾑ     <          2                             ﾚ
      ﾙ  ﾂ       ｶ .      ﾇ 7    ｷ                               -   ﾒ   ｹ >          3                  ﾅ          ﾛ
      ﾚ  ﾃ       ｷ =        8  ﾐ ｸ                               <   ﾓ   ｺ ｱ          4                  ﾆ      ﾕ   ﾜ
      ﾛ  ﾄ       ｸ *        9  ﾑ                                         ｻ ｲ          5                  ﾇ      ﾖ   ﾝ
      ﾜ  ﾅ       ｹ +        :  ﾒ                                         ｼ            6            ｽ     ﾈ      ﾗ   0
      ﾝ  ﾆ       ｺ -        .  ﾓ                                         ｽ                         ｾ     ﾉ      ﾘ
     ﾊ0  ﾇ       ｻ <        =  ﾔ                                         ｾ                         ｿ     ﾊ      ﾙ
     ﾋ1          ｼ >        *  ﾕ                                         ｿ                         ﾀ     ﾋ      ﾚ
     ﾌ2          ｽ ｱ        + ｸﾖ                                         ﾀ                         ﾁ     ﾌ      ﾛ
     ﾍ3          ｾ ｲ          ｹﾗ                                         ﾁ                ﾑ        ﾂ     ﾍ      ﾜ
     ﾎ4      ﾊ   ｿ ｳ          ｺﾘ                                         ﾂ                ﾒ        ﾃ     ﾎ      ﾝ
     ﾏ5      ﾋ     ｴ          ｻﾙ                                                          ﾓ        ﾄ            0
  ⠀⠑⢼⡧⠊⠀     ﾌ     ｵ          ｼﾚ                                                          ﾔ        ﾅ            1
  ⠀⠀⠎⠱⠀⠀  done     ｶ           ﾛ                                                          ﾕ        ﾆ
             ﾎ     ｷ                                                                               ﾇ
 > Describe what you want to build…                                                                ﾈ
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts                                                ctx 3.6k (sin ventana)
```

## 2. Cabecera de filtrado en el transcript

Detalle del mismo turno: la cabecera lleva la regla y la ruta del original íntegro.

```text
 loom · mock-model · ready · ODD     1  + ﾒ   ﾃ       ｵ  ｽﾙ                     ｳ       ｴ      ﾙ  ﾈ       ﾒ   ﾎ    .
 ╭» 👤                 ﾅﾌ                -      ﾗ         ｾ                ﾑ             ｵ Session                  =
 ╰─ ejecuta el ruido  ﾆﾍ                <      ﾘ         ｿ                ﾒ             ｶ responding · 0.0s        *
 ╭─ ● bash  (for /L %i in (1,1,40) do @echo ruido) & echo FIN             ﾓ             ｷ step 2/3                 +
 │  │ [filtrado: 39 líneas omitidas de 41 — regla general; el original sigue en  ﾍ      ｸ0     ﾝ                   -
 │  C:\work\.loomcoding\tool-output\tool-call-ba ｹ1Tokens                  ｵ<
 │  sh-ruido-1794092772914066200-0.txt]      ﾚ ﾜ         ﾃ                       ﾏ ﾜ    ｺ2↑7164 ↓30               ｶ>
 │  │ ruido ×40       ﾋ                     ｾﾛ ﾝ         ﾄ                       ﾐ ﾝ    ｻ3     2                  ｷｱ
 ╰─   … (1 lines hidden) [Ctrl+O to expand] ｿﾜ 0         ﾅ                       ﾑ 0    ｼ4MCP not configured · LSPｸｲ
 ╭≋ 🤖         3       ﾍ                     ﾀ  1         ﾆ                   9   ﾒ 1     5rust-analyzer found · skills ﾎ
 ╰─ Hecho: la salida ruidosa llegó filtrada al modelo.                       :   ﾓ 2     60/0 cards, 0 scopes     ｺｴ   ﾏ
 ｴ  ｺ         5                             ﾂ  3                             .   ﾔ 3     7                        ｻｵ   ﾐ
 ｵ  ｻ         6                            8ﾃ  4           ｷ                 =   ﾕ 4     8Tools                   ｼ    ﾑ
 ｶ  ｼ         7                  .         9ﾄ  5           ｸ                 *   ﾖ 5      ● bash                  ｽ    ﾒ
 ｷ  ｽ         8                  =         :ﾅ  6       0   ｹ    ﾘ3                 6                              ｾ    ﾓ
 ｸ  ｾ         9                  *         .ﾆ  7       1   ｺ    ﾙ4                 7                              ｿ    ﾔ
 ｹ  ｿ         :                  +         =ﾇ  8       2   ｻ    ﾚ5                 8                              ﾀ    ﾕ
 ｺ  ﾀ         .                  -         *           3   ｼ    ﾛ6                 9                              ﾁ    ﾖ
 ｻ  ﾁ    ｺ    =           ｿ      <         +           4   ｽ    ﾜ7                                                     ﾗ
 ｼ  ﾂ    ｻ                ﾀ      >         -           5   ｾ    ﾝ8                                                     ﾘ
 ｽ       ｼ                ﾁ      ｱ                     6   ｿ    09         .                                        ﾕ
 ｾ       ｽ                ﾂ      ｲ                     7   ﾀ     :         =                                        ﾖ
 ｿ       ｾ       ｲ        ﾃ      ｳ                     8   ﾁ     .         *                                        ﾗ
         ｿ       ｳ        ﾄ      ｴ                     9   ﾂ     =   ﾏ     +                                        ﾘ
         ﾀ       ｴ        ﾅ 5    ｵ                     :         *   ﾐ     -                                        ﾙ
      ﾘ  ﾁ       ｵ        ﾆ 6    ｶ                     .         +   ﾑ     <          2                             ﾚ
      ﾙ  ﾂ       ｶ .      ﾇ 7    ｷ                               -   ﾒ   ｹ >          3                  ﾅ          ﾛ
      ﾚ  ﾃ       ｷ =        8                                    <   ﾓ   ｺ ｱ          4                  ﾆ      ﾕ   ﾜ
      ﾛ  ﾄ       ｸ *        9  ﾑ                                     ﾔ   ｻ            5            ｼ     ﾇ      ﾖ   ﾝ
      ﾜ  ﾅ       ｹ +        :  ﾒ                                         ｼ            6            ｽ     ﾈ      ﾗ   0
      ﾝ  ﾆ       ｺ -        .  ﾓ                                         ｽ                         ｾ     ﾉ      ﾘ   1
      0  ﾇ       ｻ <        =  ﾔ                                         ｾ                         ｿ     ﾊ      ﾙ
     ﾋ1          ｼ >        * ｷﾕ                                         ｿ                         ﾀ     ﾋ      ﾚ
     ﾌ2          ｽ ｱ        + ｸﾖ                                         ﾀ                ﾐ        ﾁ     ﾌ      ﾛ
     ﾍ3          ｾ ｲ        - ｹﾗ                                         ﾁ                ﾑ        ﾂ     ﾍ      ﾜ
  ⠀⠢⣰⣆⠔⠀         ｿ ｳ          ｺﾘ                                         ﾂ                ﾒ        ﾃ     ﾎ      ﾝ
  ⠀⢀⠜⠣⡀⠀  done     ｴ          ｻﾙ                                                          ﾓ        ﾄ            0
     ﾐ6      ﾌ     ｵ           ﾚ                                                          ﾔ        ﾅ            1
 > Describe what you want to build…                                                                ﾆ
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts                                                ctx 3.6k (sin ventana)
```

## 3. Turno sin nabled = true (con RUST_LOG puesta)

Sin la sección, la salida llega tal cual al modelo: no hay cabecera, no hay colapso y el registro no recibe ninguna línea
del destino output_filter.

```text
 loom · mock-model · ready · ODD     3  < ﾔ   ﾅﾘ    ﾓ ｷ  ｿﾛ               ﾒ     ｵ       ｶ      ﾛ  ﾊ 1     ﾔ   ﾐ    *
 ╭» 👤                 ﾇﾎ                >      ﾙ         ﾀ                ﾓ             ｷ Session                  +
 ╰─ ejecuta el ruido  ﾈﾏ                ｱ    ﾘ ﾚ         ﾁ                ﾔ             ｸ responding · 0.0s        -
 ╭─ ● bash  (for /L %i in (1,1,40) do @echo ruido) & echo FIN             ﾕ      ﾎ      ｹ1step 2/3                 <
 │  │ ruido           ﾊﾑ                     ﾚ ﾜ         ﾃ                ﾖ      ﾏ      ｺ2     1                  ｶ>
 │  │ ruido   1       ﾋ                      ﾛ ﾝ         ﾄ                       ﾐ ﾝ    ｻ3Tokens                  ｷｱ
 ╰─   … (39 lines hidden) [Ctrl+O to expand]ｿﾜ 0         ﾅ                       ﾑ 0    ｼ4↑7183 ↓30               ｸｲ
 ╭≋ 🤖         3       ﾍ                     ﾀﾝ 1         ﾆ                       ﾒ 1     5     4                  ｹｳ
 ╰─ Hecho: la salida ruidosa llegó filtrada al modelo.                       :   ﾓ 2     6MCP not configured · LSPｺｴ   ﾏ
 ｴ  ｺ         5                             ﾂ  3                             .   ﾔ 3     7rust-analyzer found · skills ﾐ
 ｵ  ｻ         6                             ﾃ  4                             =   ﾕ 4     80/0 cards, 0 scopes     ｼｶ   ﾑ
 ｶ  ｼ         7                             ﾄ  5                             *   ﾖ 5     9                        ｽ    ﾒ
 ｷ  ｽ         8                            :ﾅ  6           ｹ                 +   ﾗ 6     :Tools                   ｾ    ﾓ
 ｸ  ｾ         9                  *         .ﾆ  7       1   ｺ                 -     7      ● bash                  ｿ    ﾔ
 ｹ  ｿ         :                  +         =ﾇ  8       2   ｻ    ﾚ5                 8                              ﾀ    ﾕ
 ｺ  ﾀ         .                  -         *ﾈ  9       3   ｼ    ﾛ6                 9                              ﾁ    ﾖ
 ｻ  ﾁ         =                  <         +ﾉ          4   ｽ    ﾜ7                 :                              ﾂ    ﾗ
 ｼ  ﾂ         *           ﾀ      >         -           5   ｾ    ﾝ8                                                     ﾘ
 ｽ  ﾃ    ｼ                ﾁ      ｱ         <           6   ｿ    09                                                     ﾙ
 ｾ       ｽ                ﾂ      ｲ         >           7   ﾀ    1:                                                  ﾖ  ﾚ
 ｿ       ｾ                ﾃ      ｳ                     8   ﾁ    2.         *                                        ﾗ
 ﾀ       ｿ       ｳ        ﾄ      ｴ                     9   ﾂ     =         +                                        ﾘ
         ﾀ       ｴ        ﾅ      ｵ                     :   ﾃ     *   ﾐ     -                                        ﾙ
         ﾁ       ｵ        ﾆ 6    ｶ                     .   ﾄ     +   ﾑ     <                                        ﾚ
         ﾂ       ｶ        ﾇ 7    ｷ                     =         -   ﾒ     >          3                             ﾛ
      ﾚ  ﾃ       ｷ =      ﾈ 8    ｸ                               <   ﾓ   ｺ ｱ          4                             ﾜ
      ﾛ  ﾄ       ｸ *      ﾉ 9    ｹ                               >   ﾔ   ｻ ｲ          5                  ﾇ          ﾝ
      ﾜ  ﾅ       ｹ +        :  ﾒ                                 ｱ   ﾕ   ｼ ｳ          6                  ﾈ      ﾗ   0
      ﾝ  ﾆ       ｺ -        .  ﾓ                                         ｽ            7            ｾ     ﾉ      ﾘ   1
      0  ﾇ       ｻ <        =  ﾔ                                         ｾ                         ｿ     ﾊ      ﾙ   2
      1  ﾈ       ｼ >        *  ﾕ                                         ｿ                         ﾀ     ﾋ      ﾚ
     ﾌ2  ﾉ       ｽ ｱ        +  ﾖ                                         ﾀ                         ﾁ     ﾌ      ﾛ
     ﾍ3          ｾ ｲ        - ｹﾗ                                         ﾁ                         ﾂ     ﾍ      ﾜ
     ﾎ4          ｿ ｳ        < ｺﾘ                                         ﾂ                ﾒ        ﾃ     ﾎ      ﾝ
     ﾏ5          ﾀ ｴ          ｻﾙ                                         ﾃ                ﾓ        ﾄ     ﾏ      0
  ⠀⠢⣰⣆⠔⠀     ﾌ     ｵ          ｼﾚ                                                          ﾔ        ﾅ     ﾐ      1
  ⠀⢀⠜⠣⡀⠀  done     ｶ          ｽﾛ                                                          ﾕ        ﾆ            2
      8      ﾎ     ｷ           ﾜ                                                          ﾖ        ﾇ            3
 > Describe what you want to build…                                                                ﾈ
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts                                                ctx 3.6k (sin ventana)
```

## Qué comprueba la prueba

- El modelo recibe la cabecera con la regla general y la ruta del original.
- El original íntegro (40 líneas uido y FIN) existe en la ruta que anuncia la cabecera.
- El registro del daemon (<perfil>/.loomcoding/logs/loomcoding-daemon.log) contiene una línea del destino
  output_filter con egla="general", original_bytes y sent_bytes.
- Sin nabled = true: ni cabecera en el modelo ni en el transcript, ni línea de registro.