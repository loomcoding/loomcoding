# CodeLoom — Invocación directa de agentes por la persona usuaria, con selección de skills

> Estado: **propuesta**, pendiente de aprobación (2026-11-01). Sin código hasta que spec y plan estén aprobados.
>
> Origen: `references/2026-11-01-estudio-plugins-ecc-ponytail-superpowers.md` §5. La persona usuaria quiere que la
> conversación principal quede limpia y que cada especialidad (`superpowers`, `ponytail`…) sea un **agente** que ella invoca,
> con las skills que le interesen en ese momento, en su propia sesión y contexto, visible dentro de la conversación principal.
> Decidido en la conversación: se admiten **las dos sintaxis**, `/agent` y `@`, como complementarias.
>
> Relacionado: `hechos/2026-10-22-codeloom-a2a-agent-registry-design.md` (registro y aprobación),
> `2026-11-01-codeloom-external-catalog-profiles.md` (catálogo externo), `2026-10-31-codeloom-skill-size-policy.md`
> (topes de skills), `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`.

## 1. Problema

Hoy solo el **modelo** puede lanzar un subagente (herramienta `subagent`). La persona usuaria puede pedírselo
(«usa el agente superpowers…»), pero la petición pasa por el modelo principal, que la reformula, gasta un turno de la
conversación principal y decide por su cuenta el prompt del hijo. Además, las skills de un agente van fijas en su perfil
(`scope.skills`): no se puede lanzar el mismo agente con otro subconjunto de skills.

## 2. Objetivo

Que la persona usuaria pueda lanzar un agente del registro **directamente**, con su propio texto como prompt y,
opcionalmente, un subconjunto de las skills que el perfil declara; que el hijo aparezca en la conversación principal como un
nodo A2A más, sin tocar el prompt principal.

## 3. No objetivos

- No se inyecta nada en el prompt de la conversación principal.
- No se permite cargar skills que el perfil **no** declara: la aprobación por huella cubre el perfil y las skills que cita, y
  una skill ajena quedaría sin aprobar.
- No se cambia el protocolo A2A ni los estados de tarea: un agente invocado así es una tarea normal del padre.
- Sin agentes ni catálogos configurados, no cambia nada.
- No se mide ahorro ni calidad.

## 4. Diseño

### 4.1 Sintaxis (dos, equivalentes)

- `/agent <nombre> [--skills a,b,c] <tarea>`: orden explícita, con autocompletado del nombre y de las skills, como las demás
  órdenes con barra.
- `@<nombre> [--skills a,b,c] <tarea>`: atajo. Solo se reconoce si el compositor **empieza** por `@` y el nombre coincide con un
  agente usable del registro; en cualquier otro caso el texto es un mensaje normal (un `@fichero` no se confunde con un agente).

Las dos producen la misma orden interna. Se interpretan al **enviar**, nunca por pulsación: el compositor no hace trabajo extra
al escribir. El autocompletado de nombres usa la lista de agentes que la interfaz ya tiene en memoria, sin leer disco.

### 4.2 Orden interna

Nueva `ClientCommand::RunAgent { agent, skills, prompt }` de la interfaz al servidor. El servidor la ejecuta contra la sesión
abierta, que hace de padre, mediante una nueva operación del lanzador, `start_requested`, que recibe una `SubagentRequest`
(etiqueta, prompt, agente, `skills: Option<Vec<String>>`). Con `skills = None` se comporta exactamente como `start`.

### 4.3 Selección de skills

`skills` es un **subconjunto ordenado** de `scope.skills` del perfil. Se valida **antes** de crear la tarea:

- una skill que el perfil no declara → error visible, no se crea tarea;
- máximo 16 (el tope vigente de skills por agente), sin repetidos;
- un agente sin `scope.skills` rechaza cualquier selección.

Sin `--skills`, se cargan todas las declaradas (comportamiento de hoy). El resultado de la carga de cada skill sigue el flujo
vigente (`SkillsLoaded`, topes de tamaño).

### 4.4 Qué se ve y qué no

- El hijo es una tarea del padre: nodo con el estado A2A exacto, etiqueta `[agente·origen]`, y las skills cargadas.
- La persona usuaria escribe `@superpowers diseña X` y en el diario aparece **su** mensaje y el nodo; el modelo principal **no**
  corre un turno por ello.
- Si el hijo pide algo a la persona usuaria (`input-required`), la pregunta le llega como hoy.
- Al terminar, el resultado queda en el nodo y en la sesión del hijo, a la que se entra desde el carril. **No** se despierta al
  modelo principal por defecto (ver pregunta abierta 1).

### 4.5 Aprobación, errores y cotas

- Solo agentes usables (integrados, globales o aprobados). Un agente pendiente de aprobación se rechaza con el mismo mensaje
  que ya produce `start`, orientando a `/agents`.
- Un nombre inexistente en `/agent` da error; en `@` se trata como mensaje normal (§4.1).
- Cota del prompt: la de la herramienta `subagent`. Cota del nombre: 64. Etiqueta: derivada de nombre y primeras palabras.
- Respeta `max_active_subagents`, el techo de profundidad y el presupuesto de la familia, porque usa el mismo lanzador.
- `delegation = "off"` retira la herramienta al modelo, **no** impide que la persona usuaria invoque un agente: es una orden
  suya, no una delegación del modelo.

## 5. Criterios de aceptación

1. `/agent rust revisa X` y `@rust revisa X` crean la misma tarea, con el prompt exacto de la persona usuaria y sin turno del
   modelo principal.
2. `@algo` con un nombre que no es agente se envía como mensaje normal.
3. `--skills a,b` carga solo esas, si están declaradas; una no declarada, o repetida, o más de 16, falla antes de crear tarea.
4. Un agente pendiente de aprobación se rechaza con orientación; uno aprobado de catálogo funciona.
5. El nodo aparece con estado A2A y etiqueta de origen; `input-required` del hijo llega a la persona usuaria.
6. Sin `--skills`, el comportamiento de `subagent` y de los perfiles existentes no cambia.
7. El compositor no lee disco ni hace trabajo nuevo por pulsación (el análisis es al enviar; el autocompletado usa memoria).
8. Las cuatro comprobaciones del proyecto pasan. Todo con proveedor simulado, sin red.

## 6. Riesgos

- **Confusión con el modelo:** si la persona usuaria espera que el modelo principal «sepa» lo que hizo el agente, no es así
  hasta que lea el resultado. Por eso se deja abierta la pregunta del despertar.
- **Secuestro del compositor:** una sintaxis con `@` puede chocar con texto legítimo. Se mitiga reconociendo solo nombres
  existentes y solo al principio.
- **Coste oculto:** una invocación directa gasta tokens del hijo (y de sus nietos si el perfil delega). Cuenta en la misma
  familia de presupuesto y respeta `hard_stop`.

## 7. Preguntas abiertas

1. ¿Debe el modelo principal enterarse del resultado (despertarlo al terminar el hijo), como con una delegación suya?
   Propuesta: no por defecto, para mantener la conversación principal limpia; un indicador por invocación podría activarlo.
2. ¿Selección de skills solo por texto (`--skills`) o también con una ventana? Propuesta: solo texto en esta fase.
3. ¿`/agent` sin tarea abre una sesión interactiva con el agente (entrar en él), en lugar de ejecutar una tarea? Propuesta:
   fuera de esta fase.
