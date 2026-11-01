# CodeLoom — Web de ayuda con mdBook

> Estado: **propuesta**, pendiente de aprobación (2026-11-01). Sin código ni andamiaje hasta que spec y plan estén aprobados.
>
> Origen: la persona usuaria quiere documentar todo el repositorio como una web de ayuda. Sus decisiones constan en §3.
>
> Relacionado: `docs/README.md` (índice del diseño interno, no se sustituye), `docs/inventario.md`,
> `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`, `docs/visual-e2e.md`.

## 1. Intención

Hoy no existe documentación de uso. El `README.md` raíz está desfasado (afirma que MCP y skills «no están implementados») y
`docs/` es diseño interno: specs, planes y estudios. Nadie puede aprender a usar, configurar o entender CodeLoom sin leer código.

Se quiere un **libro mdBook** dentro del repositorio que se entienda **de un vistazo**, con esquemas, listas numeradas y tablas
antes que prosa, y que **no se desfase** porque lo que sale del código lo genera el código.

## 2. Objetivos, no objetivos y anti-objetivos

**Objetivos**
1. Un libro con tres partes: **Usar**, **Configurar** y **Características**, más portada, glosario y hojas de consulta.
2. Cada concepto se capta de un vistazo y se ve cómo cruza con los demás (mapa de conceptos).
3. Las fichas de herramientas, claves de configuración, órdenes y estados A2A salen **del código** y un test falla si el libro se queda viejo.
4. Los ejemplos de configuración y de perfiles se **verifican** con el cargador real.
5. El libro se construye y se verifica en CI en cada cambio y se sube como **artefacto**. La publicación en GitHub Pages queda para cuando el repositorio sea público (§9, fase 4).

**No objetivos (esta entrega)**
- La parte **Contribuir** (arquitectura de crates, pruebas, E2E visual, herdr): fase 2.
- El **inglés**: fase 3. La estructura queda preparada.
- Sustituir `docs/README.md` ni mover specs y planes: el diseño sigue donde está y el libro lo enlaza.
- Documentar lo que no existe (memoria, OAuth interactivo, TLS propio): consta solo como «Límites conocidos».

**Anti-objetivos (sería un fracaso aunque compile)**
- Párrafos largos. Si una página necesita prosa para entenderse, falta un esquema.
- Un libro que describa lo que se pretendía y no lo que hace el código.
- Una referencia escrita a mano que envejece en semanas.
- Presentar como A2A algo que es decisión de CodeLoom.

## 3. Decisiones

**De la persona usuaria**
- mdBook de Rust, dentro del repositorio.
- Español primero, preparado para añadir inglés.
- Primera entrega: **Usar** + **Configurar** + **Características**. Características incluye las herramientas internas y la
  comunicación entre el prompt principal y los subagentes.
- Características se explica **por dentro**, con flujos y diagramas, y además con **parámetros y ejemplos**.
- Estilo: esquemas, numeración, puntos; poca prosa; conceptos transversales de un vistazo.

**Mías (rotuladas; cámbialas si no te cuadran)**
- Ubicación `docs/book/` (fuente en `docs/book/src/`, salida en `docs/book/book/`, ignorada por git).
- Diagramas en **Mermaid** con `mdbook-mermaid`: texto en el repositorio, revisable en un diff.
- Plantilla fija de página y tope de «una pantalla» (§5).
- Portada con **tres puertas** y el **mapa de conceptos** a la vista; buscador de mdBook; hojas de consulta en la barra lateral.
- Generación desde el código mediante un test que escribe con `UPDATE_BOOK=1` y compara en el resto de ejecuciones (§6).
- Un job aparte `book` en CI que construye, verifica y sube el libro como artefacto. El despliegue a Pages se deja escrito pero **desactivado** hasta que haya repositorio público y dirección.
- Licencia del libro: la del repositorio (MIT o Apache-2.0).
- **Versionado por release** (decidido por la persona usuaria; el mecanismo es mío): el libro lleva en la cabecera la versión del
  workspace y el commit; el artefacto de CI se nombra con esa versión; al publicar (fase 4) cada etiqueta `vX.Y.Z` se despliega
  en `/vX.Y.Z/` y `/latest/` apunta a la última. mdBook no versiona de serie, así que es construir una vez por etiqueta.

## 4. Estructura del libro

```
Portada ............ tres puertas + mapa de conceptos
1 Usar
  Instalación · Primer arranque · Proveedores y modelos
  La TUI (conversación, paneles, carril de subagentes)
  Órdenes y atajos · Sesiones y cola de prompts
  Contexto, compactación y coste · Lanzar un agente (/agent, @)
  Límites conocidos
2 Configurar
  codeloom.toml (referencia generada)
  Perfiles de agente · Skills · MCP · Catálogos externos
  Agentes remotos A2A (consumir y servir)
  Delegación y presupuestos · Aprobación y seguridad
3 Características
  Mapa de conceptos · El bucle del agente
  Herramientas (índice + una ficha por herramienta, generadas)
  Comunicación prompt principal ↔ subagentes
  Estados de tarea A2A · Jobs · Eventos y observabilidad
  Contexto y coste · Caché de prompt
Apéndices .......... glosario · hojas de consulta · enlaces al diseño
```

## 5. Plantilla de página

1. **Qué es:** una línea.
2. **Esquema:** diagrama Mermaid o tabla.
3. **Pasos o reglas:** lista numerada.
4. **Parámetros:** tabla (nombre, tipo, valor por defecto, efecto).
5. **Ejemplo:** bloque verificable.
6. **Ver también:** enlaces, incluidos los del diseño.

Reglas: una página cabe en una pantalla (si no, se parte); lo que es de A2A y lo que es decisión nuestra va rotulado, con
el estado A2A exacto entre paréntesis; las advertencias de seguridad van en un recuadro al principio de la página que las necesita.

## 6. Contenido generado y verificado

| Qué | De dónde sale | Cómo se vigila |
|---|---|---|
| Fichas de herramientas (nombre, descripción, esquema, paralela o exclusiva) | el registro de herramientas real | test: el fichero generado coincide con el registro |
| Claves de `codeloom.toml` | las estructuras de configuración | test: toda clave documentada existe y toda clave pública está documentada |
| Órdenes con barra | `COMMANDS` de `codeloom-tui` | test: la tabla coincide |
| Estados A2A | `TaskState` | test: los ocho estados con su nombre de cable |
| Ejemplos `toml` y de perfil | los propios `.md` del libro | test: se extraen y se cargan con el cargador real |

Los ficheros generados viven en `docs/book/src/generated/` y llevan una cabecera que lo avisa. Cómo se orquesta (test, `xtask` o
binario auxiliar) se deja al constructor, con una condición: sin red y dentro de las cuatro comprobaciones.

## 7. Seguridad (sección propia, enlazada desde Usar y Configurar)

1. Las claves van en `.env`, nunca en `codeloom.toml`.
2. Un binario real sin `CODELOOM_HOME` temporal se conecta al daemon real de la persona usuaria.
3. Los agentes de proyecto, de catálogo y remotos piden **aprobación por huella**; ningún perfil de proyecto añade servidores MCP.
4. `delegation = off` es la única garantía de código contra la delegación; el resto son reglas de prompt.
5. Qué hace y qué no hace cada presupuesto (`hard_stop`, topes de salida).

## 8. Restricciones

- Las cuatro comprobaciones (`build`, `test`, `fmt --check`, `clippy -D warnings`) pasan en cada commit.
- Sin red en tests; el libro se construye en un job aparte.
- Instalar `mdbook` y `mdbook-mermaid` es una dependencia de desarrollo: se pide permiso al instalarlas, no antes.
- El corpus de terceros no se copia al libro; se enlaza la página original.
- No se tocan `docs/README.md` ni los ficheros sin commitear de la persona usuaria.
- Se actualiza el `README.md` raíz para que no mienta y apunte al libro (es parte de esta entrega).
- Se conservan los finales de línea de cada fichero que se edite.

## 9. Fases

1. **Esta entrega:** esqueleto, plantilla, generación y verificación, Usar, Configurar, Características, seguridad, glosario,
   hojas de consulta, README actualizado, CI y Pages.
2. **Contribuir:** arquitectura de crates, pruebas, E2E visual, herdr, protocolo A2A.
3. **Inglés:** segunda versión con el mismo esqueleto.
4. **Publicación:** activar el despliegue a Pages cuando el repositorio sea público y se decida la dirección.

## 10. Criterios de aceptación

1. `mdbook build` termina sin avisos y produce un sitio con buscador y diagramas dibujados.
2. Toda página sigue la plantilla de §5 y no pasa de una pantalla.
3. Cada herramienta del registro tiene su ficha generada: las 20 de la sesión raíz y `artifact_publish`, que solo ven los hijos. Las de MCP son dinámicas y se documentan como categoría, no una a una. Añadir una herramienta sin regenerar rompe un test.
4. Todos los ejemplos `toml` y de perfil del libro se cargan con el cargador real; uno inválido rompe un test.
5. La página de comunicación con subagentes cubre lanzamiento, contexto del hijo, resultado, `input-required`, despertar del
   padre, cancelación, avances y recursión, con los estados A2A exactos.
6. El `README.md` raíz ya no contradice el código y enlaza el libro.
7. El job `book` de CI pasa y sube el libro como artefacto. El despliegue a Pages existe pero está desactivado.
8. El libro muestra la versión del workspace y el commit, y el artefacto de CI lleva esa versión en el nombre.
9. Las cuatro comprobaciones del proyecto pasan.

## 11. Lo que queda al constructor

- El mecanismo exacto de generación y comparación (§6), mientras sea offline y esté dentro de las cuatro comprobaciones.
- La convención para marcar bloques verificables en Markdown (por ejemplo, la etiqueta del bloque).
- La paleta y el tema de mdBook, siempre que los diagramas y el modo oscuro se lean bien.
- El reparto fino de páginas dentro de cada parte, respetando el tope de una pantalla.

## 12. Preguntas abiertas

Resueltas el 2026-11-01 por la persona usuaria:

1. El repositorio **todavía no es público** y no tiene Pages: la publicación se aplaza (fase 4).
2. La **dirección final** está pendiente de decidir; no bloquea esta entrega.

Resuelta también el 2026-11-01: **sí, se versiona por release** (§3).

Ninguna sigue abierta.
