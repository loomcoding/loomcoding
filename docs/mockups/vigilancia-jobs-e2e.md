# Volcado real de la vigilancia de jobs

Capturado con `cargo test -p loomcoding-cli --test tui_pty_vigilancia` (perfil aislado con `LOOMCODING_HOME`
temporal, proveedor `wiremock` con el guion `mock_provider_watch`, 40×120). Es una pantalla real de la TUI.

Lo que muestra la captura: el `job_start` con `watch` (patrón `FAIL`) arranca el job `vigilado`; su línea
`FAIL uno` casa y sale en el transcript como línea del actor «job» (`job: [Job shell-1 watch] FAIL uno`,
con su origen `MessageOrigin::Job`; el aviso de despertar sigue como `sys:`); el modelo despierta
(«auto-turn · woken by «job watch»») y contesta «aviso recibido». La tarjeta del job y la sección Jobs del
rail muestran `watching: 1 alert`, una nota propia de Loomcoding, sin rótulo de estado A2A.

```text loom · mock-model · ready · ODD · 0 subagents running · 0 jobsｽﾙ                     ｳ       ｴ      ﾙ  ﾈ ﾝ     ﾒｽ  ﾎ    .
 ╭» 👤                 ﾅﾌ                -      ﾗ         ｾ                ﾑ             ｵ Session                  =
 ╰─ vigila los fallos del build         <      ﾘ         ｿ                ﾒ             ｶ responding · 0.0s        *
 ╭─ ● job_start  echo FAIL uno          >    ﾗ ﾙ         ﾀ                ﾓ             ｷ step 1/3                 +
 ╰─ │ Started background job shell-1. Watching output for /FAIL/ (up to 5 notices).     ｸ0     ﾝ                   -
 ╭─ ◆ job  vigilado   ﾉﾐ                     ﾙ ﾛ         ﾂ                ﾕ      ﾎ      ｹ1Tokens                  ｵ<
 │  │ watching: 1 alert                      ﾚ ﾜ         ﾃ                       ﾏ ﾜ    ｺ2↑10657 ↓37              ｶ>
 ╰─ │ exit 0  1       ﾋ                     ｾﾛ ﾝ         ﾄ                       ﾐ ﾝ    ｻ3     2                  ｷｱ
 ╭≋ 🤖         2       ﾌ                     ｿﾜ 0         ﾅ                       ﾑ 0    ｼ4MCP not configured · LSPｸｲ
 ╰─ job lanzado, vigilo la salida           ﾀ  1         ﾆ                   9   ﾒ 1     5rust-analyzer found · skills ﾎ
 ── job: [Job shell-1 watch] FAIL uno       ﾁ  2                             :   ﾓ 2     60/0 cards, 0 scopes     ｺｴ   ﾏ
 ─◌ sys: ⟳ auto-turn · woken by «job watch» (new lines)                      .   ﾔ 3     7     6                  ｻｵ   ﾐ
 ╭≋ 🤖         6                             ﾃ  4                             =   ﾕ 4     8Tools                   ｼｶ   ﾑ
 ╰─ aviso recibido                         9ﾄ  5           ｸ                 *   ﾖ 5     9● job_start             ｽ    ﾒ
 ｷ  ｽ         8                  =         :ﾅ  6       0   ｹ                 +     6                              ｾ    ﾓ
 ｸ  ｾ         9                  *         .ﾆ  7       1   ｺ    ﾙ4                 7      Jobs                    ｿ    ﾔ
 ｹ  ｿ         :                  +         =ﾇ  8       2   ｻ    ﾚ5                 8      ● vigilado              ﾀ    ﾕ
 ｺ  ﾀ         .                  -         *ﾈ          3   ｼ    ﾛ6                 9        watching: 1 alert     ﾁ    ﾖ
 ｻ  ﾁ         =           ｿ      <         +           4   ｽ    ﾜ7                                                     ﾗ
 ｼ  ﾂ    ｻ                ﾀ      >         -           5   ｾ    ﾝ8                                                     ﾘ
 ｽ       ｼ                ﾁ      ｱ         <           6   ｿ    09                                                     ﾙ
 ｾ       ｽ                ﾂ      ｲ                     7   ﾀ    1:         =                                        ﾖ
 ｿ       ｾ       ｲ        ﾃ      ｳ                     8   ﾁ     .         *                                        ﾗ
         ｿ       ｳ        ﾄ      ｴ                     9   ﾂ     =   ﾏ     +                                        ﾘ
         ﾀ       ｴ        ﾅ      ｵ                     :   ﾃ     *   ﾐ     -                                        ﾙ
      ﾘ  ﾁ       ｵ        ﾆ 6    ｶ                     .         +   ﾑ     <          2                             ﾚ
      ﾙ  ﾂ       ｶ .      ﾇ 7    ｷ                               -   ﾒ   ｹ >          3                             ﾛ
      ﾚ  ﾃ       ｷ =      ﾈ 8    ｸ                               <   ﾓ   ｺ ｱ          4                  ﾆ          ﾜ
      ﾛ  ﾄ       ｸ *        9                                    >   ﾔ   ｻ ｲ          5            ｼ     ﾇ      ﾖ   ﾝ
      ﾜ  ﾅ       ｹ +        :  ﾒ                                         ｼ            6            ｽ     ﾈ      ﾗ   0
      ﾝ  ﾆ       ｺ -        .  ﾓ                                         ｽ                         ｾ     ﾉ      ﾘ   1
      0  ﾇ       ｻ <        =  ﾔ                                         ｾ                         ｿ     ﾊ      ﾙ   2
      1  ﾈ       ｼ >        * ｷﾕ                                         ｿ                         ﾀ     ﾋ      ﾚ
     ﾌ2          ｽ ｱ        + ｸﾖ                                         ﾀ                ﾐ        ﾁ     ﾌ      ﾛ
     ﾍ3          ｾ ｲ        - ｹﾗ                                         ﾁ                ﾑ        ﾂ     ﾍ      ﾜ
  ⠀⠢⣰⣆⠔⠀         ｿ ｳ        < ｺﾘ                                         ﾂ                ﾒ        ﾃ     ﾎ      ﾝ
  ⠀⢀⠜⠣⡀⠀  done     ｴ          ｻﾙ                                                          ﾓ        ﾄ     ﾏ      0
     ﾐ6      ﾌ     ｵ          ｼﾚ                                                          ﾔ        ﾅ            1
 > Describe what you want to build…                                                                ﾆ            2
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts                                                ctx 3.6k (sin ventana)
```

