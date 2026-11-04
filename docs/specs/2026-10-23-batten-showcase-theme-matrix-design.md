# Matriz de componentes por tema en el showcase de Cloth

Estado: implementado el 2026-10-23, sin verlo en una terminal real (véase «Estado» al final).

## Propósito

Saber de un vistazo **qué componente existe en qué tema**, sin recorrer los demos uno a uno. Hoy el
showcase es una lista de 130 demos (61 componentes) y la única forma de saber si «Composer» existe
en Pawan es buscarlo a mano.

## Decisiones (de la persona usuaria, 2026-10-23)

1. **Filas** = componentes, a la izquierda. **Columnas** = temas, en una cabecera para
   seleccionarlos.
2. **Tres estados por celda**, tal cual:
   - `●` hay reproducción propia de ese tema.
   - `◐` solo existe el widget genérico, pintado con la paleta del tema.
   - `·` no existe.
3. Columnas: Cloth (el aspecto *classic*), Martty, Mindfork, Pawan, Crush, MastraCode y
   **Loomcoding**, que la persona usuaria considera «otro tema más». *Interpretación propia:*
   «classic» es Cloth, porque ese nombre solo existe como `Look::classic()`; y Mindfork se
   incluye aunque no se nombró.
4. **`●` sin número** (elección delegada a mí): el recuento de demos de la celda se muestra en el
   pie al situarse encima.
5. **dotmax no es un tema, es una librería**: va a una nota al pie, no a una columna.
7. **Loomcoding debe tener su propia paleta** (respuesta de la persona usuaria a la pregunta
   abierta): se añade `ThemePreset::Loomcoding`. Véase «Paleta de Loomcoding».
6. **Cambiar de tema dentro del propio componente** (petición posterior): al ver un componente se
   puede pasar de un tema a otro de su fila para compararlo, y el pie dice en qué estado está
   (propio, parcialmente implementado, no existe). Véase «Vista de un componente entre temas».

## Qué hay hoy (medido el 2026-10-23 sobre `demos/mod.rs`)

El catálogo ya declara `component`, `variant` y `provider` por demo, así que la matriz se deriva
de datos que existen; no hace falta adivinar por nombre de fichero. Cifras: 130 demos, 61
componentes.

- **MastraCode no tiene ningún demo propio**: es solo una paleta. Su columna saldría sin un solo
  `●`, y eso es información, no un fallo.
- **Crush** es la columna con más reproducciones (decenas de componentes); Martty, Mindfork y
  Pawan tienen pocas.
- **Loomcoding** tiene 12 demos (todos de herramientas) pero **no existe `ThemePreset::Loomcoding`**:
  hoy es solo un proveedor de demos, sin paleta propia. **dotmax** (3 demos) es una librería.

## Diseño

### Origen de los datos

Una función pura sobre `catalog()` que devuelve, por componente y tema, un `Cell`:
`Own(n)` (n demos del proveedor de ese tema), `Generic` o `Missing`.

- `Own(n)` si hay al menos un demo con ese `component` y el proveedor del tema.
- `Generic` si no hay propio **pero existe un demo del proveedor Cloth** para ese componente: se
  asume que ese widget se pinta con la paleta activa porque `Demo::render` recibe el `Theme`.
- `Missing` en el resto. **No se promociona** un demo de otro proveedor a `Generic`: no consta que
  respete el tema activo (p. ej. un demo de Crush puede llevar sus colores fijos).

### Estricta con el proveedor (hallazgo)

`provider_for` devuelve «Cloth» **por defecto** cuando el nombre de la variante no empieza por un
proveedor conocido. Hoy eso afecta a 6 entradas: cinco con componente «Cloth — primitivas
exclusivas» y variantes Button, File changes, Toast, Effects y Themes, y una, «Tool activity»,
con la variante «Cross-reference comparison». La matriz **no usa ese valor por defecto**: usa una
variante de `provider_for` que devuelve `Option` y deja esas entradas fuera de las columnas (en una
nota al pie), de modo que un demo mal nombrado no infle la columna de Cloth en silencio.

### La columna Loomcoding y la nota de dotmax

Loomcoding es una columna más y, con `ThemePreset::Loomcoding`, aplica su propia paleta como las
demás. *Mientras no existiera el preset* la columna abría el demo con el tema activo; ese
comportamiento desaparece al añadirlo.

dotmax (Braille graphics, Progress catalog) es una librería: sus componentes van a una nota al
pie, no a la matriz, para no llenarla de `·` que no significan «falta» sino «no es un tema».

### Interfaz

La matriz es un **modo del showcase**, no un `Demo`: necesita ver el catálogo entero y cambiar el
tema y la selección, y el trait `Demo` no ve nada de eso. Se abre desde el índice.

- Flechas: mover la celda. Cabecera con los seis temas.
- `Enter` sobre una celda: abre el demo de ese componente **con ese tema aplicado**. Si la celda
  es `Own(n)` con `n > 1`, abre el primero; si es `Generic`, abre el demo de Cloth con el tema de
  la columna; si es `Missing`, no hace nada y lo dice en la línea de ayuda.
- Esc: vuelve a la matriz desde el demo.
- La tecla de ciclar tema que ya existe no cambia.
- Pie con la leyenda `● propio  ◐ genérico  · no existe` y el recuento por columna.

### Pruebas

- La derivación es pura y se prueba sin terminal: `crush_composer_is_own`,
  `pawan_composer_is_generic`, `mastracode_has_no_own_cells`,
  `an_unknown_provider_is_left_out_not_counted_as_cloth`,
  `generic_needs_a_cloth_demo` (un componente solo de Crush no es `Generic` en Pawan).
- Que todo `Own` y `Generic` apunta a un demo que existe en el catálogo.
- Un `TestBackend` de 100×40 que pinta la matriz y contiene los tres símbolos y los seis temas.

### Vista de un componente entre temas

Con un demo abierto, `F8` / `Shift+F8` pasa a la **columna siguiente / anterior de la fila de su
componente** y cambia a la vez el tema y el demo: `Own` abre el demo de ese proveedor con su
paleta; `Generic` abre el de Cloth con la paleta de esa columna. Las columnas `Missing` se
**saltan** (no hay nada que ver; la fila las muestra como `·`). La columna Loomcoding no cambia la
paleta (no tiene preset). `F7` sigue siendo «solo la paleta», sin cambiar de demo.

El **pie de dos líneas** se muestra si el componente tiene fila en la matriz:

1. La fila del componente: los siete nombres de columna con su símbolo, la actual resaltada:
   `Cloth ● · Martty ● · Mindfork ● · Pawan ◐ · Crush ● · MastraCode ◐ · Loomcoding ◐`.
2. La nota de la columna actual:
   - `Own`: «● propio de {tema}» (con «n variantes» si `n > 1`).
   - `Generic`: «◐ parcialmente implementado: no hay demo de {tema}; se muestra el genérico de
     Cloth con la paleta de {tema}».
   - Loomcoding: «sin paleta propia: se usa el tema activo».

Sin fila (componentes fuera de la matriz) el pie conserva su línea actual. La columna actual es
la de la última navegación con `F8` o `Enter` desde la matriz; al elegir otro demo desde el
índice se deduce del proveedor del demo.

## Paleta de Loomcoding

Hasta ahora Loomcoding era solo un proveedor de demos; el preset `Cloth` es la paleta neutra de
Cloth (cian y violeta), no la de Loomcoding. La nueva paleta parte de lo que **ya existe** en el
repositorio: el logo (`crates/loomcoding-cloth/src/widgets/logo.rs`) usa el naranja `#c25a27` y el pizarra
`#5c6370`.

| Rol | Valor | Origen |
|---|---|---|
| acento | `#c25a27` (194, 90, 39) | logo |
| borde | `#5c6370` (92, 99, 112) | logo |
| usuario | `#e2844e` (226, 132, 78) | propio: el naranja aclarado |
| fondo | `#14151a` (20, 21, 26) | propio |
| texto | `#dee0e4` (222, 224, 228) | propio |
| atenuado | `#8b919c` (139, 145, 156) | propio: el pizarra aclarado, para que se lea sobre el fondo |
| asistente | igual que texto | propio |
| aviso | `#e5c07b` (229, 192, 123) | propio |
| error | `#e06c75` (224, 108, 117) | propio |
| éxito | `#98c379` (152, 195, 121) | propio |

Solo el acento y el borde son de marca; **el resto son decisiones propias**, fáciles de cambiar
en una sola tabla. El aspecto (`Look`) es el clásico, igual que Cloth. No se afirma que sea «la»
identidad visual de Loomcoding: es una propuesta construida sobre los dos colores del logo.

## Fuera de alcance, declarado

- **Comprobar que un demo ajeno respeta el tema activo.** No se verifica; por eso no cuenta como
  `Generic`.
- **Ocultar o reordenar filas** por filtro; la primera versión las lista todas en el orden del
  catálogo.
- **Corregir las 6 entradas mal nombradas**: la matriz las deja fuera con un aviso. Renombrarlas
  es una tarea aparte.

## Preguntas abiertas

- ¿Gusta la paleta propuesta? Solo el acento y el borde salen del logo.

## Estado

> **Reemplazado en parte** por `2026-10-23-batten-showcase-theme-first-design.md`: la persona
> usuaria declaró errónea la vista «un componente con todos los temas dentro». Quedan obsoletos
> `F8` / `Shift+F8`, la tira de la fila y la nota del pie de «Vista de un componente entre
> temas», la fila de proveedores como filtro y la tecla `p`. Siguen vigentes la matriz (`m`), su
> derivación, los tres estados de celda y la paleta de Loomcoding.

Implementado el 2026-10-23. **Sin verlo aún en una terminal real**: solo hay pruebas con
`TestBackend`, de modo que colores, anchos y desplazamiento están sin comprobar a ojo. Commits:
`a5dd92d` (derivación pura), `b2a102a` (modo matriz y teclas), `c64870f` (dibujo), `aeb2317`
(`F8` y pie con la nota) y `7adbd30` (paleta de Loomcoding).

### Cómo verlo

`cargo run -p loomcoding-cloth --example showcase`, y desde el índice: `m` abre la matriz; flechas o
`hjkl` mueven el cursor; `Enter` abre el demo de la celda con el tema de su columna; `Esc` vuelve.
Con un demo abierto, `F8` / `Shift+F8` cambia de tema dentro de su componente y `F7` solo la
paleta.

### Qué sale hoy (medido el 2026-10-23, 57 filas)

| Tema | Propio `●` | Genérico `◐` | No existe `·` |
|---|---|---|---|
| Cloth | 21 | 0 | 36 |
| Martty | 8 | 18 | 31 |
| Mindfork | 10 | 15 | 32 |
| Pawan | 9 | 17 | 31 |
| Crush | 38 | 14 | 5 |
| MastraCode | 0 | 21 | 36 |
| Loomcoding | 1 | 21 | 35 |

Los dos componentes de dotmax (Progress catalog, Braille graphics) van a la nota del pie, y dos
más quedan fuera por tener un proveedor no reconocido («Tool activity» y el grupo «Cloth ·
primitivas exclusivas»), con un recuento en la misma nota.

### Decisiones tomadas durante la implementación

- **La nota manda sobre el nombre del tema**: con la nota completa («parcialmente implementado»)
  el sufijo `F7/Shift+F7 tema: …` solo se añade si cabe entero. La primera versión recortaba la
  nota para conservar el sufijo y perdía lo más útil. El texto de la nota genérica se acortó
  respecto al de la spec.
- **`F8` es «la siguiente columna con algo»**: desde Crush va a MastraCode, no a Pawan; para
  llegar a Pawan desde Crush se usa `Shift+F8`.
- **El pie de dos líneas aparece también en el índice y en los filtros**, no solo con el demo
  abierto.
- **`ThemePreset::Loomcoding` está en `ThemePreset::ALL`**, así que también entra en el ciclo de
  temas de la aplicación y se guarda en las preferencias como `theme = "loomcoding"`.

### Lo que no consta o queda pendiente

- **La paleta de Loomcoding**: solo el acento y el borde salen del logo; el resto es propuesta.
  No consta que sea la identidad visual de Loomcoding.
- **`Generic` supone** que el demo de Cloth respeta el tema activo porque `Demo::render` recibe
  el `Theme`. No se ha comprobado demo a demo.
- **Los nombres largos se recortan** en la columna de componentes con 100 columnas
  («Assistant…»), porque las siete cabeceras completas ocupan casi todo el panel.
- **`Option` en `COLUMNS`**: tras añadir el preset de Loomcoding todas las columnas llevan preset y
  el `Option` es ya innecesario; se dejó para no ampliar el cambio.
- **Seis entradas mal nombradas** del catálogo siguen sin corregir; la matriz las deja fuera.
- **`clippy --example showcase` falla con 5 lints preexistentes** (`Showcase::len` sin uso fuera
  de tests, un `else { if }` en `draw`, y tres demos). El comando del plan no los alcanza.
