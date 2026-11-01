# Especificación: límites de tamaño para skills locales

Estado: **implementada** (2026-11-01) en el cargador y en la composición del prompt del hijo; véase la nota de implementación al final.

## Objetivo

Hacer que CodeLoom pueda cargar skills locales válidas de tamaño habitual sin confundir bytes con las recomendaciones de longitud de Agent Skills, y mantener un límite explícito al contexto añadido por agente.

## Hallazgos

- El loader de **crates/codeloom-core/src/skills.rs** rechaza un **SKILL.md** por encima de 4 KiB y limita el prompt agregado a 6 KiB. También limita el frontmatter a 2 KiB y la selección a 16 skills por agente.
- Git sitúa la introducción de esos valores en **18ac61bf** (feat: composer con pegados largos, adjuntos de imagen, atribución, skills locales y supervisión de uso, 2026-10-31). El mensaje describe la carga de skills como «explícita y acotada», pero no explica por qué se eligieron 4 KiB ni 6 KiB. La especificación anterior los registró como límites, sin documentar su origen. No he encontrado evidencia de que 4 KiB venga de A2A o de Agent Skills.
- La [especificación de Agent Skills](https://agentskills.io/specification) recomienda mantener el SKILL.md principal por debajo de 500 líneas y alrededor de 5.000 tokens; aconseja mover material detallado a archivos de referencia y cargarlos solo cuando hagan falta. Son guías de contenido y divulgación progresiva, no un máximo normativo de 4 KiB ni una regla para rechazar cualquier skill que pase de 500 líneas.
- En la copia local de ECC, ejemplos del directorio skills/ que exceden 4 KiB son rust-patterns (13.500 bytes, 500 líneas), python-patterns (16.914 bytes, 751 líneas), typescript-patterns (23.655 bytes, 592 líneas), frontend-patterns (15.432 bytes, 657 líneas) y coding-standards (12.866 bytes, 551 líneas). El tope actual impide cargarlos aunque estén declarados explícitamente en el scope.
- CodeLoom estima texto como bytes UTF-8 divididos entre cuatro. La cifra se muestra como estimación, no como conteo real del proveedor.

Conclusión: 4 KiB es una decisión local de implementación para acotar el prompt; no es una exigencia de protocolo o formato. Cambiarlo por un rechazo rígido en la línea 501 tampoco sería compatible con varias skills de ECC ni con el carácter recomendado, no obligatorio, de esa guía.

## Política propuesta

1. Mantener la carga explícita desde scope.skills, la validación de IDs y raíces, la prioridad de la skill del proyecto, el máximo de 16 skills y el límite de 2 KiB de frontmatter.
2. Elevar el límite defensivo de lectura por archivo de 4 KiB a 32 KiB. Es una cota propia de CodeLoom para acotar lectura y memoria, no un requisito de Agent Skills. Con el estimador actual equivale aproximadamente al presupuesto agregado propuesto.
3. Sustituir el límite agregado de 6 KiB por un máximo de 8.000 tokens estimados por agente. Es un presupuesto local inicial: permite una skill ECC de hasta unos 5.914 tokens estimados y deja margen para otras instrucciones, mientras mantiene acotado el prompt. La selección que exceda el presupuesto conserva el estado visible PromptLimit; no se trunca silenciosamente una skill.
4. Tratar las 500 líneas y los 5.000 tokens por SKILL.md como objetivos recomendados para autoría. No rechazar una skill solo por pasar de 500 líneas si cumple las cotas locales de lectura y prompt.
5. Seguir cargando solamente el SKILL.md enumerado. No descubrir ni cargar automáticamente references/, ejecutar scripts, instalar dependencias, registrar tools ni cargar skills remotas en este cambio.
6. Seguir mostrando el consumo como estimación informativa. El presupuesto de skills limita el prompt de skills; no altera el presupuesto general de la tarea, los permisos ni el uso informado por el proveedor.

## Criterios de aceptación para la implementación

1. Una skill explícita válida de más de 4 KiB se puede cargar si cabe en las cotas nuevas; incluir fixtures representativos como rust-patterns, python-patterns y typescript-patterns.
2. Pasar de 500 líneas no provoca por sí solo Oversized.
3. Un archivo que supera 32 KiB se informa como Oversized sin cargarlo.
4. El contenido agregado que supera 8.000 tokens estimados se informa como PromptLimit, sin superar el presupuesto ni insertar contenido parcial.
5. Solo se leen IDs declarados y rutas dentro de las raíces aprobadas; la precedencia proyecto/usuario permanece igual.
6. Los tokens atribuidos coinciden con el texto que efectivamente se añade al prompt, incluido el prefijo compartido una sola vez.
7. Se verifica que la carga sigue siendo determinista y acotada con pruebas locales, sin red ni ejecución del contenido de una skill.

## Alcance y límites

Esta especificación propone corregir los topes locales del loader. No modifica A2A, AgentCard, el selector de agentes, CodeLoom en ejecución ni el formato de las skills. La carga bajo demanda de archivos de referencia queda fuera de esta corrección.

El estimador actual es una aproximación de bytes UTF-8 divididos entre cuatro; por tanto, los 8.000 tokens son una cota estimada, no una garantía exacta frente al tokenizer de cada proveedor. Si CodeLoom incorpora más adelante un tokenizer específico o un presupuesto dinámico de contexto, podrá sustituir esa estimación conservando las cotas y estados visibles.

## Nota de implementación

La spec no recogía dos topes aguas abajo que habrían anulado el cambio, y se corrigieron a la vez:

- `AgentLoop` recortaba todo el prompt adicional del hijo (persona del perfil más skills) a 16.000 bytes
  (`MAX_SYSTEM_EXTRA_BYTES`): unas skills de hasta 32.000 bytes habrían llegado cortadas por el medio, con los tokens
  atribuidos contando texto que no se enviaba. El tope pasa a ser persona (16.000) + separador + skills (32.000) + una
  holgura de 128 bytes para el marcador de recorte.
- `subagent.rs` restaba el tamaño de las skills a los 16.000 bytes de la persona: con skills grandes la persona se
  quedaba en cero. Ahora cada parte tiene su cota propia (`compose_child_system`).

