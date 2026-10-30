# Plan: marcador para pegados largos

Spec: [`docs/specs/2026-10-29-codeloom-large-paste-placeholder.md`](../specs/2026-10-29-codeloom-large-paste-placeholder.md)

## Alcance

Mostrar un marcador para cada evento `Paste` de más de 1000 caracteres y
expandir el contenido original, sin cambios, al preparar el mensaje enviado.
Mantener separados los elementos de pegado de texto literal idéntico al
marcador.

## Pasos

1. Revisar los usos de `batten::state::Composer` y elegir una representación de
   pegado atómico compatible con los editores de cola y ajustes ya existentes.
2. Insertar pegados largos como elementos con identidad, etiqueta visible y
   payload íntegro en memoria; conservar pegados cortos como texto ordinario.
3. Adaptar el render, movimiento del cursor, selección y borrado para respetar
   los límites del elemento sin cambiar la edición de texto normal.
4. Expandir todos los elementos al preparar el envío y guardar el texto
   expandido en historial/sesión para que una recuperación no pierda el payload.
5. Añadir pruebas de umbral, Unicode, orden, colisiones literales, edición,
   borrado y expansión en el mensaje; ejecutar las comprobaciones requeridas.

## Estado

- [x] Revisar los usos de `Composer` y limitar el pegado colapsado al chat.
- [x] Insertar pegados largos como elementos con identidad y conservar el payload.
- [x] Adaptar movimiento, selección, copia, borrado y envío.
- [x] Añadir pruebas de umbral, Unicode, orden, texto literal y borrado atómico.
- [x] Ejecutar la suite de `batten`, la prueba de integración de `App`, formato y clippy.
- [ ] Repetir `cargo build` y `cargo test` del workspace cuando Windows permita reemplazar `target/debug/codeloom.exe`; verificar la presentación en la TUI real.

## Riesgos y límites

- Un marcador puramente visual no reduce el tamaño del mensaje ni el consumo de
  tokens. La expansión antes de enviar es obligatoria para no perder contexto.
- Los índices de cursor y selección deben permanecer correctos al insertar,
  borrar o reordenar texto alrededor de uno o varios elementos atómicos.
