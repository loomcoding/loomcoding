# Ayuda y documentación de usuario — spec

## Problema

Loomcoding tiene documentación **técnica** (arquitectura, specs, mapa de código) pero nada pensado para **quien lo usa**.
Dentro de la app, `/help` solo lista atajos de teclado. Quien llega nuevo no encuentra cómo empezar, qué hace cada ajuste
de `loomcoding.toml` ni qué comandos hay. La referencia de calidad que se quiere igualar es la documentación de Mastra:
corta, escaneable, con ejemplos y con una guía por tarea.

## Intención

Tres entregas, **en este orden**, con una sola fuente de verdad:

1. **Documentación de usuario** (`docs/user/`, Markdown en el repo): empezar, conceptos, guías por tarea, referencia y
   solución de problemas.
2. **Ayuda dentro de la app**: `/help` por temas y por comando, alimentada por la misma fuente que la referencia, con
   enlaces a las páginas.
3. **Sitio web de documentación**: publica las mismas páginas Markdown, con navegación y buscador. Sin duplicar texto.

## Reglas de estilo (obligatorias, se comprueban)

- **Párrafos de 4 líneas como máximo.** Si hace falta más, es una lista o una tabla.
- **Esquematizar:** listas, tablas, pasos numerados y diagramas antes que prosa.
- **Ejemplo primero, explicación después.** Cada página abre con *Qué es · Para qué · Cómo*, en 5 líneas como mucho.
- **Una idea por página.** Si se hace larga, se parte.
- **Imágenes solo si ahorran texto:** una por concepto, con texto alternativo. Diagramas en Mermaid (se versionan y se
  diffean); capturas de la TUI generadas a partir de volcados reales, nunca a mano.
- Títulos que respondan a una pregunta o a una acción («Cambiar de modelo», no «Modelos»).
- **Sin datos personales:** ni usuario de Windows, ni rutas absolutas de la máquina, ni correos ni nombres.
- Todo ajuste y todo comando documentado indica su **valor por defecto** y si **requiere reiniciar**.

## Decisiones de la persona usuaria (2026-11-08)

1. **Bilingüe, con el inglés por defecto.** `docs/user/en/` es la versión de referencia y `docs/user/es/` su espejo en
   español, con las mismas páginas y la misma estructura. `docs/user/README.md` (el índice) está en inglés y enlaza a las
   dos. El tono es neutro y claro en ambos idiomas (sin regionalismos ni jerga).
2. **El sitio es `loomcoding.com`**: un sitio propio de Loomcoding. El registro del dominio, el DNS y el alojamiento los
   hace la persona usuaria; el repositorio aporta la construcción y la documentación de cómo publicarlo.
3. **Capturas en SVG** generado a partir del texto del volcado del e2e (ligero y diffeable). Si el experimento de la
   tarea 5 demuestra que no se lee bien, se reabre.

## Estructura de `docs/user/{en,es}/`

La misma estructura en los dos idiomas (los nombres de carpeta y de fichero son los mismos):

| Carpeta | Contenido |
|---|---|
| `empezar/` | Instalar, primer arranque, configurar un proveedor (en la nube y local), primer prompt |
| `conceptos/` | Agente, subagentes y jobs («flock»), sesiones, contexto y compactación, permisos, flujos (ODD), A2A |
| `guias/` | Una tarea por página: modelos locales, agentes de perfil, agentes remotos, vigilar un job, compresión de salida, medir con `RUST_LOG`, atajos |
| `referencia/` | Comandos `/`, atajos de teclado, `loomcoding.toml` (todas las secciones), variables de entorno, carpetas del perfil |
| `problemas/` | Síntoma → causa → arreglo |

## Una sola fuente de verdad

- Los comandos `/` salen de `COMMANDS` (`crates/loomcoding-tui/src/commands.rs`); los ajustes, de las estructuras de
  `crates/loomcoding-core/src/config/`.
- **Paridad de idiomas:** una prueba falla si una página existe en `en/` y no en `es/` (o al revés), o si sus títulos de
  sección no coinciden en número.
- Una **prueba** (`cargo test`) falla si hay un comando o un ajuste sin documentar en `referencia/`, o documentado y
  desaparecido del código. Es lo que evita que la documentación se pudra.
- Otra prueba comprueba el estilo: párrafos largos, enlaces rotos, datos personales y páginas sin la apertura
  *Qué es · Para qué · Cómo*.

## Qué no cambia

- La documentación técnica (`docs/ARCHITECTURE.md`, `CODE-MAP.md`, specs) sigue siendo para quien desarrolla; las páginas
  de usuario **enlazan** a ella solo cuando hace falta, no la copian.
- Ningún cambio de comportamiento del producto salvo `/help` (entrega 2). A2A se describe tal cual es: lo propio de
  Loomcoding se rotula como propio.

## Riesgos

- **Se desincroniza con el código:** mitigado por las pruebas de sincronía; no basta con la disciplina.
- **Capturas que envejecen:** se generan desde los volcados del e2e (`docs/visual-e2e.md`), con un script que las
  regenera; una captura sin su script no entra.
- **Sitio web = infraestructura:** se hace al final y reutiliza el Markdown, para que el repo siga siendo la fuente.

## Preguntas abiertas

1. **Alojamiento de `loomcoding.com`.** Propuesta: GitHub Pages con el dominio propio (lo más simple y gratis). Se confirma
   al llegar a la entrega 3; no bloquea las dos primeras.
2. **Traducción de los términos del producto** (flock, jobs, subagentes): se mantienen en inglés en ambos idiomas, como
   en la interfaz, con una frase de explicación la primera vez.
