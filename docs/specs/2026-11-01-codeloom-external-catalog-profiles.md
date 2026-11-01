# CodeLoom — Catálogo externo de agentes y skills como perfiles locales

> Estado: **propuesta**, pendiente de aprobación (2026-11-01). Sin código hasta que spec y plan estén aprobados.
>
> Contexto: la persona usuaria mantiene fuera de este repositorio un catálogo de agentes, skills y patrones de terceros
> (`E:\workspace\personal\loom-agent-workforce-poc`, «el catálogo») para que CodeLoom no se vuelva gigante. El reparto es:
> el catálogo elige y compone; **CodeLoom es el runtime** que ejecuta. Decidido en la conversación: los agentes del
> catálogo entran en CodeLoom como **perfiles locales**, no como agentes A2A remotos.
>
> Relacionado: `hechos/2026-10-22-codeloom-a2a-agent-registry-design.md` (registro y aprobación),
> `2026-10-15-loom-agent-profile-a2a.md` (formato del perfil), `2026-10-31-codeloom-skill-size-policy.md` (topes de
> skills), `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md` (todo con cota).

## 1. Problema

El catálogo ya exporta ficheros con el formato de perfil de CodeLoom (`.agents/agents/<nombre>.md`, con `scope.skills` y
`a2a.card.skills`) y las skills que esos perfiles citan (`.agents/skills/<id>/SKILL.md`). Hoy CodeLoom solo lee perfiles de
dos carpetas: la global (`~/.agents/agents`, de confianza) y la del proyecto (`.agents/agents`, con aprobación). Un
catálogo de terceros no encaja en ninguna:

- Copiarlo a la carpeta global lo saltaría la aprobación, siendo contenido ajeno.
- Copiarlo a cada proyecto duplica ficheros y obliga a mantenerlos al día.
- Las skills del catálogo no están en las raíces de skills de CodeLoom, así que `scope.skills` no las encontraría.

## 2. Objetivo

Que CodeLoom pueda usar los agentes y skills de uno o varios catálogos externos **sin copiarlos y sin acoplarse a su
implementación**, con las garantías que ya tiene un agente de proyecto: aprobación por huella, origen visible y todo
acotado.

## 3. No objetivos

- CodeLoom **no lee** `catalog.json`, `agent-pack.json` ni nada propio del catálogo, ni lo ejecuta. El único contrato son
  ficheros de perfil y de skill. Si el catálogo cambia por dentro, CodeLoom no se entera ni le importa.
- No se decide qué agente conviene a una tarea: eso es del catálogo (su flujo de decisión). CodeLoom solo ejecuta el
  agente que se le nombra, por `subagent { agent }` o por el enrutador existente.
- No cambia el protocolo A2A ni se declara ningún perfil importado «compatible con A2A» por llevar tarjeta.
- No se concede ningún permiso por lo que el perfil importado declare: las pistas de herramientas o de modelo de terceros
  son datos, no concesiones (el catálogo ya las deja sin importar).
- Sin catálogos configurados, comportamiento idéntico al actual.

## 4. Diseño

### 4.1 Configuración

Nueva clave opcional `[agents] catalog_roots = ["<ruta>", ...]`: lista de raíces de catálogo (absolutas; se admite `~`).
De cada raíz se leen `<raíz>/.agents/agents/*.md` y `<raíz>/.agents/skills/<id>/SKILL.md`, la misma disposición que el
catálogo ya produce. Lo que no exista se ignora sin error. Cota: como mucho 8 raíces.

### 4.2 Nivel de confianza `catalog`

Se añade un tercer nivel de perfil, `ProfileLevel::Catalog`, que se comporta como el de proyecto en lo que importa:

- **Siempre requiere aprobación** (clave: raíz canónica del catálogo, nombre, huella), con el mismo almacén
  `agent-trust.json`. Sin aprobar, el agente no se usa ni aparece en la sección «Available agents» (aparece como pendiente,
  como hoy los de proyecto). Así, un catálogo de 72 perfiles no desborda el prompt: solo cuentan los aprobados.
- Origen visible: `catalog:<ruta>` en `agent_origin` del descriptor, `(catalog)` en la sección de agentes del prompt y
  `[nombre·catalog]` en la etiqueta del nodo de la interfaz, junto a los `·project` y `·remote` actuales.
- Precedencia ante un nombre repetido: integrado < global < catálogo < proyecto. Un duplicado se anota como `Shadowed`,
  como ya hace el registro.

### 4.3 Skills del catálogo

`scope.skills` de un perfil de catálogo se resuelve primero en las raíces habituales (proyecto, usuario) y, si no está, en
`<raíz del catálogo>/.agents/skills`, **solo para ese perfil**: una skill del catálogo no queda disponible para el resto de
agentes. Se aplican los topes vigentes de la política de tamaño de skills.

Riesgo y cierre: la aprobación cubre el fichero del agente, no sus skills, y estas son contenido de terceros que podría
cambiar después. Por eso la huella de un agente de catálogo se calcula sobre **el fichero del agente más el contenido de las
skills que cita** (en el orden de `scope.skills`). Cambiar un byte de cualquiera invalida la aprobación.

### 4.4 Tarjeta A2A

`a2a.card.skills` del perfil ya llega a `AgentProfile.card_skills`. No se cambia cómo se sirve ni se publica: un agente de
catálogo no se anuncia como agente A2A propio por estar en el catálogo. Queda fuera de esta spec decidir si las tarjetas
servidas deben derivar sus `skills` de los perfiles (hoy van vacías); es una decisión aparte.

### 4.5 Cotas (decálogo)

- Raíces de catálogo: 8. Ficheros de perfil leídos por raíz: 200. Tamaño de un perfil: el tope vigente de perfiles.
- Skills por perfil: las que declare `scope.skills`, con los topes de la política de skills.
- La sección «Available agents» sigue en 8.000 bytes; los pendientes se resumen en una línea con su número.
- La recarga en caliente por sondeo vigila las carpetas del catálogo con el mismo intervalo; un catálogo ausente no cuesta
  nada.

### 4.6 Observabilidad y diario

- El origen `catalog:<ruta>` queda en el descriptor y se ve en el nodo del subagente, igual que el de proyecto.
- Las llamadas a `subagent` con un agente de catálogo son visibles en el diario como cualquier otra, con el actor
  distinguible por su etiqueta.
- La ventana de aprobación (`/agents`) muestra los agentes de catálogo agrupados y con su origen; aprobar o revocar es
  por agente, nunca en bloque (ver pregunta abierta 1).

## 5. Criterios de aceptación

1. Con `catalog_roots` vacío o ausente, nada cambia (pruebas existentes intactas).
2. Un perfil de una raíz de catálogo se carga con nivel `Catalog` y queda **pendiente**; no aparece como disponible hasta
   aprobarlo.
3. Aprobado, aparece en «Available agents» con `(catalog)` y `subagent { agent }` lo lanza; el descriptor guarda
   `catalog:<ruta>`.
4. Cambiar un byte del perfil, o de una skill que cita, invalida la aprobación.
5. Una skill del catálogo se carga para su perfil y no para otro agente.
6. Un nombre repetido respeta la precedencia y anota `Shadowed`.
7. Las cotas se cumplen: más de 8 raíces o 200 perfiles recortan con aviso, sin fallar.
8. La etiqueta del nodo en la interfaz muestra `·catalog`.
9. Las cuatro comprobaciones del proyecto pasan. Todo con proveedor simulado, sin red.

## 6. Riesgos

- **Contenido de terceros en el prompt:** un perfil de catálogo es texto ajeno que acaba en el prompt del hijo. Lo mitigan
  la aprobación por huella (incluidas sus skills), el origen visible y que el catálogo ya trata sus textos como no fiables.
  No es una garantía: la persona usuaria aprueba lo que lee.
- **Fatiga de aprobación:** aprobar muchos agentes uno a uno cansa; la tentación sería aprobar en bloque. Se deja como
  pregunta abierta y por defecto no se hace.
- **Acoplamiento por disposición de carpetas:** si el catálogo cambia `.agents/…`, CodeLoom deja de verlo. Es el único
  contrato y se documenta.
- **Rutas entre unidades:** las raíces son absolutas; el catálogo del usuario vive en `E:` y los clones de referencia en
  `G:`. Nada relativo.

## 7. Preguntas abiertas

1. ¿Se permite una aprobación en bloque (por catálogo y huella global), o solo una a una? Propuesta: solo una a una.
2. ¿La huella de un agente de catálogo debe incluir también los ficheros de apoyo de las skills (`checks.md`)? Propuesta:
   sí, si la skill los declara.
3. ¿Quién avisa a CodeLoom cuando el catálogo exporta de nuevo? Propuesta: nadie; la recarga por sondeo ya lo detecta.
