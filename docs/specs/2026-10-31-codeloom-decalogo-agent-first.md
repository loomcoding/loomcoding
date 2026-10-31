# CodeLoom — Decálogo agent-first (borrador) y diseño de evaluación

> Estado: **borrador para discusión**. Los principios son candidatos extraídos de decisiones ya
> tomadas en el proyecto y contrastados con fuentes externas; no están adoptados. Ninguna de las
> afirmaciones de eficacia está medida todavía.
>
> Fecha: 2026-10-31
>
> Relacionado: `2026-10-25-codeloom-agent-first-development-skills.md` (principio y skills),
> `2026-10-25-codeloom-agent-development-workflow.md` (flujo),
> `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md` (decálogo previo, de contexto y coste),
> `docs/references/2026-10-31-agent-first-fuentes-externas-y-auditoria.md` (fuentes y auditoría).

## 1. Propósito

API-first se resume en una idea: el contrato se diseña antes que la implementación y es el artefacto
que gobierna a todos los consumidores. Se propone un decálogo análogo para un arnés cuyo consumidor
principal es un agente. Debe poder defenderse en dos sentidos: como guía de diseño del propio
CodeLoom y como contribución publicable, lo cual exige separar con claridad lo que es práctica
local, lo que es fuente externa y lo que está medido.

Alcance distinto del decálogo existente: el de contexto y coste acota lo que llega al modelo; este
cubre cómo se diseña un sistema en el que agentes actúan, delegan y son supervisados.

## 2. Candidatos

Columnas: **Local** indica dónde se aplica ya en CodeLoom; **Externo** la fuente que lo respalda o
matiza; **Medido** si existe evidencia propia (hoy ninguna).

| # | Principio | Local | Externo | Medido |
|---|---|---|---|---|
| 1 | **El contrato antes que la implementación.** Perfil, herramientas permitidas y forma de entrada y salida de cada agente se declaran antes de que actúe. | Perfiles y Agent Card; flujo con spec y plan aprobados | Equivalente directo de contract-first (DHL, guías de proveedores) | No |
| 2 | **Lo estructurado es la fuente; la pantalla es una vista.** Todo estado es consumible por máquina y la TUI solo lo presenta. | Daemon, IPC y eventos; estado del flujo en SQLite | agentpatterns.ai: estructurado sobre visual | No |
| 3 | **La interfaz del agente se diseña con el rigor de la del humano.** Herramientas autodescriptivas, a prueba de errores y probadas con el modelo. | Esquemas de herramientas; rutas acotadas al proyecto | Anthropic: ACI y poka-yoke | No |
| 4 | **Nada sin cota llega al modelo.** Salidas, prompts y bucles tienen tope. | Decálogo de contexto y coste | Anthropic: simplicidad, medir | No |
| 5 | **Lo desconocido no es soportado.** Una capacidad se afirma con evidencia por destino exacto; un rechazo solo autoriza *fallback* si es de incompatibilidad. | `capability.rs` | Sin equivalente hallado | No |
| 6 | **El humano decide; el agente propone.** La aprobación se liga a la huella del contenido, falla cerrada y se invalida si el contenido cambia. | Flujo de desarrollo; aprobación de agentes de proyecto y remotos | Bandara et al.: supervisión declarada, no supuesta (solo como antecedente) | No |
| 7 | **Toda delegación es visible y reconstruible.** La conversación es el diario; los actores se distinguen. | Árbol de subagentes; estado durable | Anthropic: transparencia | No |
| 8 | **La semántica de agentes es la del estándar.** No se inventan estados; lo propio se rotula como propio. | A2A como premisa | agentpatterns.ai cita las Agent Cards de A2A | No |
| 9 | **Lo determinista, en código; lo probabilístico, donde aporta.** Transiciones, límites y bloqueos de escritura no dependen de que el modelo obedezca. | `can_transition_to`; bloqueo de escrituras en el despacho | agentpatterns.ai: determinista sobre probabilístico | No |
| 10 | **Se mide el arnés, no solo el modelo.** Toda afirmación de eficacia declara modelo, versión del arnés y entorno, y se contrasta con una ablación. | Pendiente (sección 3) | Gorinova et al. | No |

Observaciones para la discusión:

- Los puntos 5 y 6 son los que menos respaldo externo tienen y, por tanto, los más originales y los
  más necesitados de evidencia propia.
- El punto 9 solapa con el 4 y con el 6 en parte; podría fundirse. No se ha decidido.
- La simplicidad (Anthropic) se deja como criterio transversal de poda, no como principio propio:
  añadir un principio solo si mejora de forma demostrable el diseño.
- Ningún principio debe publicarse como resultado empírico. Con la evidencia actual son
  **proposiciones de diseño**.

## 3. Diseño de evaluación

### 3.1 Preguntas

1. ¿Mejoran las puertas de aprobación con huella (principio 6) la calidad final o solo añaden fricción?
2. ¿Reduce el registro de capacidades (5) los fallos de proveedor y las peticiones rechazadas repetidas?
3. ¿Qué parte de la variación de resultados se debe al arnés y qué parte al modelo (10)?

### 3.2 Método

- **Modelo fijo por serie; el eje que varía es el arnés.** Condiciones: flujo completo; sin puerta de
  aprobación; sin registro de capacidades; sin cotas de salida. Una ablación por condición frente a
  la línea base.
- **Metadatos obligatorios de cada ejecución:** modelo, versión del arnés (commit), hash del entorno,
  versión del conjunto de tareas, semilla y temperatura.
- **Tareas:** conjunto pequeño y versionado de tareas del propio repositorio, con una o más formas
  válidas de solución. Verificación por propiedades, pruebas diferenciales o criterios declarados, no
  por parche de referencia único.
- **Métricas** (las seis que ya fija la spec agent-first): cumplimiento de la aprobación, calidad de
  spec y plan, defectos del diff, verificaciones reales ejecutadas, tokens y llamadas a agentes,
  rondas de corrección. Se añade el número de peticiones rechazadas por el proveedor.
- **Repetición:** varias ejecuciones por celda y dispersión reportada; sin ella no hay conclusión.

### 3.3 Restricciones

- Las pruebas del repositorio usan siempre proveedor simulado (regla de AGENTS.md). La evaluación con
  modelo real es un procedimiento aparte, ignorado por defecto, como `anthropic_live.rs`; no entra
  en `cargo test`.
- Coste: cada celda consume tokens reales. Hay que fijar un presupuesto antes de lanzar nada y pedir
  autorización para cada lote.
- No se usan claves de otros servicios ni se imprimen secretos.

### 3.4 Amenazas a la validez

- Pocas tareas y un solo repositorio: no generaliza.
- Contaminación: el modelo pudo ver el repositorio público.
- El evaluador (revisor automático) comparte familia de modelo con el implementador.
- Las condiciones de ablación pueden dejar el arnés en un estado artificial que no refleje uso real.

### 3.5 Esquema de artículo (si se decide publicar)

1. Motivación: del contrato de API al contrato de agente.
2. Antecedentes: ACI, agent-first en la web, crítica a los benchmarks centrados en el modelo.
3. Los principios, cada uno con su decisión de diseño real en CodeLoom.
4. Evaluación por ablación (esta sección).
5. Amenazas, límites y trabajo futuro.

Sede de publicación sin decidir. Regla: sin datos de la sección 3, solo cabe un texto de posición,
con el estatus de propuesta de diseño que se indica arriba.

## 4. Decisiones abiertas

- Cuántos principios y cuáles (fusionar 4, 6 y 9; ¿se añade un principio sobre simplicidad?).
- Qué conjunto mínimo de tareas y qué presupuesto de tokens.
- Si el texto se redacta en español, en inglés o en ambos (el blog del autor es bilingüe).
- Dónde vive el borrador publicable: este repositorio guarda la spec; el texto para publicar iría a
  `proposals/` del sitio personal, según su `AGENTS.md`.
