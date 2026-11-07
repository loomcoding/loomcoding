# Volcado real de ODD en la TUI

Capturado con `cargo test -p loomcoding-cli --test tui_pty_odd -- --nocapture` (perfil aislado con `LOOMCODING_HOME` temporal, proyecto temporal con `loomcoding.toml`, proveedor `wiremock` con el guion `mock_provider_odd`, 40×120). Son pantallas reales de la TUI en texto plano, sin colores, copiadas tal cual de los ficheros que deja la prueba.

Lo que muestra cada volcado:

1. Tras `odd_track` `create` (feature `demo`, tareas `T1` y `T2`): la cabecera muestra `ODD · planning (0/2)` y la lista de tareas muestra `☐ T1` y `☐ T2`. El diálogo de permiso de `bash` sigue abierto, así que el turno queda pausado en ese punto y la etiqueta es estable.
2. Primer intento de cierre con una tarea sin probar: el gate bloquea el cierre con el aviso `Tareas pendientes: 2 — se devuelve el control al modelo (intento 1/3)` y el mensaje `[Desarrollo orgánico]` vuelve al modelo como mensaje interno.
3. Tras el `bash` (`echo ok`) y las dos marcas `done` con evidencia `call-bash-odd`: la etiqueta llega a `ODD · planning (2/2)` y las tareas aparecen `☑ T1` y `☑ T2`.

El cierre del turno (punto 4 de la prueba) no tiene volcado propio: la prueba comprueba que, tras la respuesta final y una pausa de 1,5 s, no aparece un segundo aviso ni una nueva reinyección.

Con `[flows.odd] enabled = false` la prueba `odd_disabled_shows_no_label_and_does_not_block_the_close` comprueba que no hay etiqueta `ODD · ` y que el mismo intento de cierre no genera aviso ni reinyección.

## Cómo reproducirlo con perfil aislado

```powershell
cd <repo>
cargo test -p loomcoding-cli --test tui_pty_odd -- --nocapture
```

- Cada prueba crea su propio `LOOMCODING_HOME` temporal (`Workspace`) y su proyecto temporal. No lee ni escribe el perfil real de la persona usuaria (`~/.loomcoding`), y al terminar mata sus daemons (`kill_daemons_of`).
- Los volcados quedan en `<repo>\target\tmp\e2e-shots\odd-*.txt`. Si se usa un `CARGO_TARGET_DIR` propio (véase `docs/visual-e2e.md`), la carpeta `tmp` cuelga de ese directorio.
- La prueba no necesita red ni la configuración real: el proveedor es `wiremock` en `127.0.0.1`.

## Punto 1: etiqueta y tareas tras `create`

```text
 loom · mock-model · working · ODD · planning (0/2)      ｿﾛ               ﾒ             ｶ      ﾛ  ﾊ                *
 ╭» 👤                 ﾇﾎ                >    ﾗ ﾙ         ﾀ                ﾓ      ﾌ      ｷﾝSession                  +
 ╰─ planifica la feature demo                ﾘ ﾚ         ﾁ                ﾔ      ﾍ      ｸ0waiting for provider · 0.0s
 ╭─ ● odd_track  {"feature":"demo","objective":"Demostrar ODD en la       ﾕ      ﾎ      ｹ1step 2/3                ｵ<
 │  TUI","op":"create","tasks":[…           ｽﾚ ﾜ         ﾃ                       ﾏ ﾜ     2     1                  ｶ>
 ╰─ │ Documento `demo` creado con 2 tareas. ｾﾛ ﾝ         ﾄ                   7   ﾐ ﾝ     3Tokens                  ｷｱ
 ╭─ ◆ todos  0/2                            ｿ  0                             8   ﾑ 0     4↑10755 ↓54              ｸｲ
 │  ☐ T1: primera tarea de prueba           ﾀ  1                             9   ﾒ 1     5     4                  ｹｳ   ﾎ
 ╰─ ☐ T2: segunda tarea de prueba           ﾁ  2                             :   ﾓ 2     6MCP not configured · LSPｺｴ   ﾏ
 ╭≋ 🤖         5                             ﾂ  3           ｶ                 .   ﾔ 3     7rust-analyzer found · skills ﾐ
 ╰─ Documento listo; doy el trabajo por cerrado.           ｷ                 =   ﾕ 4     80/0 cards, 0 scopes     ｼ    ﾑ
 ─◌ sys: Tareas pendientes: 2 — se devuelve el control al modelo (intento 1/3)     5                              ｽ    ﾒ
 ╭◌ sys: [Mensaje interno]       =         :ﾅ  6       0   ｹ    ﾘ3                 6      Todos  0/2              ｾ    ﾓ
 │  sys: [Desarrollo orgánico] La feature `demo` tiene 2 tareas sin terminar. No des el   ☐ T1: primera tarea de pruebaﾔ
 │  trabajo por cerrado; continúa con ellas.ﾇ          2   ｻ    ﾚ5                 8      ☐ T2: segunda tarea de pruebaﾕ
 │  sys: Pendientes:             -         *           3   ｼ    ﾛ6                 9                              ﾁ    ﾖ
 │  sys: - T1: primera tarea de prueba     +           4   ｽ    ﾜ7                        Tools                        ﾗ
 │  sys: - T2: segunda tarea de pr╭ Permiso ╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱ bash ╮    ● odd_track               ﾔ  ﾘ
 │  sys: Para cada una: márcala co│echo ok                                           │e                             ﾕ  ﾙ
 │  una llamada de bash o job de e│                                                  │                              ﾖ
 │  avanzar). Si necesitas la deci│ Permitir    Permitir en la sesión    Denegar     │                              ﾗ
 ╰─ decisión».   ｳ        ﾄ 4    ｴ│←→ elegir · enter confirmar · a/s/d atajo …       │                              ﾘ
         ﾀ       ｴ        ﾅ 5    ｵ╰──────────────────────────────────────────────────╯                              ﾙ
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
  ⠀⠀⡨⣿⢅⠀     ﾌ     ｵ          ｼﾚ                                                          ﾔ        ﾅ            1
  ⠀⠀⠀⣿⠀⠀  waiting  for you     ﾛ                                                          ﾕ        ﾆ
             ﾎ     ｷ                                                                               ﾇ
 > Describe what you want to build…                                                                ﾈ
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts                                                ctx 3.7k (sin ventana)
```

## Punto 2: cierre bloqueado y reinyección

```text
 loom · mock-model · working · ODD · planning (0/2)   ｷ  ｿﾛ               ﾒ     ｵ       ｶ      ﾛ  ﾊ       ﾔ   ﾐ    *
 ╭» 👤                 ﾇﾎ                >    ﾗ ﾙ         ﾀ                ﾓ      ﾌ      ｷﾝSession                  +
 ╰─ planifica la feature demo           ｱ    ﾘ ﾚ         ﾁ                ﾔ      ﾍ      ｸ0waiting for provider · 0.1s
 ╭─ ● odd_track  {"feature":"demo","objective":"Demostrar ODD en la       ﾕ      ﾎ      ｹ1step 2/3                 <
 │  TUI","op":"create","tasks":[…            ﾚ ﾜ         ﾃ                ﾖ      ﾏ ﾜ    ｺ2     1                  ｶ>
 ╰─ │ Documento `demo` creado con 2 tareas. ｾﾛ ﾝ         ﾄ                       ﾐ ﾝ     3Tokens                  ｷｱ
 ╭─ ◆ todos  0/2      ﾌ                     ｿﾜ 0         ﾅ                   8   ﾑ 0     4↑10755 ↓54              ｸｲ
 │  ☐ T1: primera tarea de prueba           ﾀ  1                             9   ﾒ 1     5     4                  ｹｳ
 ╰─ ☐ T2: segunda tarea de prueba           ﾁ  2                             :   ﾓ 2     6MCP not configured · LSPｺｴ   ﾏ
 ╭≋ 🤖         5                             ﾂ  3                             .   ﾔ 3     7rust-analyzer found · skills ﾐ
 ╰─ Documento listo; doy el trabajo por cerrado.           ｷ                 =   ﾕ 4     80/0 cards, 0 scopes     ｼ    ﾑ
 ─◌ sys: Tareas pendientes: 2 — se devuelve el control al modelo (intento 1/3)     5                              ｽ    ﾒ
 ╭◌ sys: [Mensaje interno]       =         :ﾅ  6       0   ｹ                       6      Todos  0/2              ｾ    ﾓ
 │  sys: [Desarrollo orgánico] La feature `demo` tiene 2 tareas sin terminar. No des el   ☐ T1: primera tarea de pruebaﾔ
 │  trabajo por cerrado; continúa con ellas.ﾇ          2   ｻ    ﾚ5                 8      ☐ T2: segunda tarea de pruebaﾕ
 │  sys: Pendientes:             -         *ﾈ          3   ｼ    ﾛ6                 9                              ﾁ    ﾖ
 │  sys: - T1: primera tarea de prueba     +           4   ｽ    ﾜ7                        Tools                   ﾂ    ﾗ
 │  sys: - T2: segunda tarea de pr╭ Permiso ╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱ bash ╮    ● odd_track                  ﾘ
 │  sys: Para cada una: márcala co│echo ok                                           │e   ● bash                    ﾕ  ﾙ
 │  una llamada de bash o job de e│                                                  │                              ﾖ  ﾚ
 │  avanzar). Si necesitas la deci│ Permitir    Permitir en la sesión    Denegar     │                              ﾗ
 ╰─ decisión».   ｳ        ﾄ      ｴ│←→ elegir · enter confirmar · a/s/d atajo …       │                              ﾘ
 ── ● bash  echo ok       ﾅ 5    ｵ╰──────────────────────────────────────────────────╯                              ﾙ
         ﾁ       ｵ        ﾆ 6    ｶ                     .         +   ﾑ     <                                        ﾚ
      ﾙ  ﾂ       ｶ .      ﾇ 7    ｷ                               -   ﾒ   ｹ >          3                             ﾛ
      ﾚ  ﾃ       ｷ =      ﾈ 8    ｸ                               <   ﾓ   ｺ ｱ          4                  ﾆ      ﾕ   ﾜ
      ﾛ  ﾄ       ｸ *        9  ﾑ                                 >   ﾔ   ｻ ｲ          5                  ﾇ      ﾖ   ﾝ
      ﾜ  ﾅ       ｹ +        :  ﾒ                                         ｼ ｳ          6            ｽ     ﾈ      ﾗ   0
      ﾝ  ﾆ       ｺ -        .  ﾓ                                         ｽ            7            ｾ     ﾉ      ﾘ   1
      0  ﾇ       ｻ <        =  ﾔ                                         ｾ                         ｿ     ﾊ      ﾙ
     ﾋ1  ﾈ       ｼ >        *  ﾕ                                         ｿ                         ﾀ     ﾋ      ﾚ
     ﾌ2          ｽ ｱ        + ｸﾖ                                         ﾀ                         ﾁ     ﾌ      ﾛ
     ﾍ3          ｾ ｲ        - ｹﾗ                                         ﾁ                ﾑ        ﾂ     ﾍ      ﾜ
     ﾎ4      ﾊ   ｿ ｳ          ｺﾘ                                         ﾂ                ﾒ        ﾃ     ﾎ      ﾝ
     ﾏ5      ﾋ     ｴ          ｻﾙ                                                          ﾓ        ﾄ     ﾏ      0
  ⠀⠀⡨⣿⢅⠀     ﾌ     ｵ          ｼﾚ                                                          ﾔ        ﾅ            1
  ⠀⠀⠀⣿⠀⠀  waiting  for you    ｽﾛ                                                          ﾕ        ﾆ
             ﾎ     ｷ           ﾜ                                                                   ﾇ
 > Describe what you want to build…                                                                ﾈ
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts                                                ctx 3.7k (sin ventana)
```

## Punto 3: dos tareas probadas, etiqueta (2/2)

```text
 loom · mock-model · working · ODD · planning (2/2) ﾑ ｵ  ｽﾙ                   4 ｳ       ｴ      ﾙ  ﾈ ﾝ     ﾒｽ  ﾎ    .
 ╭» 👤                 ﾅﾌ                -      ﾗ         ｾ                ﾑ             ｵ Session                  =
 ╰─ planifica la feature demo           <      ﾘ         ｿ                ﾒ             ｶ waiting for provider · 0.0s
 ╭─ ● odd_track  {"feature":"demo","objective":"Demostrar ODD en la       ﾓ             ｷ step 3/3                 +
 │  TUI","op":"create","tasks":[…            ﾘ ﾚ         ﾁ                ﾔ      ﾍ      ｸ0     ﾝ                   -
 ╰─ │ Documento `demo` creado con 2 tareas.  ﾙ ﾛ         ﾂ                ﾕ      ﾎ      ｹ1Tokens                  ｵ<
 ╭─ ◆ todos  2/2      ﾊ                      ﾚ ﾜ         ﾃ                       ﾏ ﾜ    ｺ2↑18214 ↓112             ｶ>
 │  ☑ T1: primera tarea de prueba           ｾﾛ ﾝ         ﾄ                       ﾐ ﾝ    ｻ3     2                  ｷｱ
 ╰─ ☑ T2: segunda tarea de prueba           ｿﾜ 0         ﾅ                       ﾑ 0    ｼ4MCP not configured · LSPｸｲ
 ╭≋ 🤖         3       ﾍ                     ﾀ  1         ﾆ                   9   ﾒ 1     5rust-analyzer found · skills ﾎ
 ╰─ Documento listo; doy el trabajo por cerrado.                             :   ﾓ 2     60/0 cards, 0 scopes     ｺｴ   ﾏ
 ─◌ sys: Tareas pendientes: 2 — se devuelve el control al modelo (intento 1/3)   ﾔ 3     7     6                  ｻｵ   ﾐ
 ╭◌ sys: [Mensaje interno]                  ﾃ  4                             =   ﾕ 4     8Todos  2/2              ｼｶ   ﾑ
 │  sys: [Desarrollo orgánico] La feature `demo` tiene 2 tareas sin terminar. No des el  9☑ T1: primera tarea de pruebaﾒ
 │  trabajo por cerrado; continúa con ellas.ﾅ  6       0   ｹ                 +     6      ☑ T2: segunda tarea de pruebaﾓ
 │  sys: Pendientes:             *         .ﾆ  7       1   ｺ    ﾙ4                 7                              ｿ    ﾔ
 │  sys: - T1: primera tarea de prueba     =ﾇ  8       2   ｻ    ﾚ5                 8      Tools                   ﾀ    ﾕ
 │  sys: - T2: segunda tarea de prueba     *ﾈ          3   ｼ    ﾛ6                 9      ● odd_track             ﾁ    ﾖ
 │  sys: Para cada una: márcala con odd_track (op done, con evidence {text, call_id} de   ● bash                       ﾗ
 │  una llamada de bash o job de esta sesión; o op block, con reason, si no puedes        ● odd_track                  ﾘ
 │  avanzar). Si necesitas la decisión de la persona usuaria, pídela y di «Necesita tu    ● odd_track                  ﾙ
 ╰─ decisión».            ﾂ      ｲ                     7   ﾀ    1:         =                                        ﾖ
 ╭─ ● bash  echo ok       ﾃ      ｳ                     8   ﾁ     .         *                                        ﾗ
 ╰─ │ ok ｿ       ｳ        ﾄ      ｴ                     9   ﾂ     =   ﾏ     +                                        ﾘ
 ╭─ ● odd_track  {"evidence":{"call_id":"call-bash-odd","text":"echo ok en -                                        ﾙ
 │  bash"},"feature":"dem…ﾆ 6    ｶ                     .         +   ﾑ     <          2                             ﾚ
 ╰─ │ T1 marcada como hecha: evidencia echo ok en bash [call: call-bash-odd].         3                             ﾛ
 ╭─ ● odd_track  {"evidence":{"call_id":"call-bash-odd","text":"echo ok en ｱ          4                  ﾆ          ﾜ
 │  bash"},"feature":"dem…  9                                    >   ﾔ   ｻ ｲ          5            ｼ     ﾇ      ﾖ   ﾝ
 ╰─ │ T2 marcada como hecha: evidencia echo ok en bash [call: call-bash-odd].         6            ｽ     ﾈ      ﾗ   0
      ﾝ  ﾆ       ｺ -        .  ﾓ                                         ｽ                         ｾ     ﾉ      ﾘ   1
      0  ﾇ       ｻ <        =  ﾔ                                         ｾ                         ｿ     ﾊ      ﾙ   2
      1  ﾈ       ｼ >        * ｷﾕ                                         ｿ                         ﾀ     ﾋ      ﾚ
     ﾌ2          ｽ ｱ        + ｸﾖ                                         ﾀ                ﾐ        ﾁ     ﾌ      ﾛ
     ﾍ3          ｾ ｲ        - ｹﾗ                                         ﾁ                ﾑ        ﾂ     ﾍ      ﾜ
  ⠀⢀⢼⡧⡰⡰         ｿ ｳ        < ｺﾘ                                         ﾂ                ﾒ        ﾃ     ﾎ      ﾝ
  ⠀⠀⡜⢣⠀⠀  thinking  working   ｻﾙ                                                          ﾓ        ﾄ     ﾏ      0
     ﾐ6      ﾌ     ｵ          ｼﾚ                                                          ﾔ        ﾅ            1
 > Describe what you want to build…                                                                ﾆ            2
enter send  ctrl+o fold  ctrl+b panel  ? shortcuts                                                ctx 3.8k (sin ventana)
```
