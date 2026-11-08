# Ayuda y documentación de usuario — spec

## Problema

Loomcoding tiene documentación **técnica** (arquitectura, specs, mapa de código) pero nada pensado para **quien lo usa**.
Dentro de la app, `/help` solo lista atajos de teclado. Quien llega nuevo no encuentra cómo empezar, qué hace cada ajuste
de `loomcoding.toml` ni qué comandos hay. La referencia de calidad que se quiere igualar es la documentación de Mastra:
corta, escaneable, con ejemplos y con una guía por tarea.

## Intención

Tres entregas, **en este orden**, con una sola fuente de verdad:

1. **Documentación de usuario** (`docs/usuario/`, Markdown en el repo): empezar, conceptos, guías por tarea, referencia y
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

## Estructura de `docs/usuario/`

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

1. **Idioma.** El README del proyecto está en inglés y las specs en español. ¿Las páginas de usuario van en español, en
   inglés o bilingües (como tu web)? Propuesta: español primero, inglés después con la misma estructura.
2. **Dónde vive el sitio.** ¿Un sitio propio de Loomcoding (por ejemplo con Docusaurus, que ya usas) o una sección de
   `valdepeace.com`? Se decide al llegar a la entrega 3; no bloquea las dos primeras.
3. **Formato de las capturas.** Propuesta: SVG generado a partir del volcado de texto (con color si el volcado lo trae),
   porque es ligero y diffeable. Alternativa: PNG de un terminal real. Se resuelve con un experimento corto en la tarea 5.
