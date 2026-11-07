# El actor «job» en el transcript — spec

## Problema

El transcript distingue quién habla: la persona usuaria, el modelo, cada subagente. Los avisos que el registro de jobs mete
en la conversación del padre (`[Job N watch] …`, `[Job N completed] …`) llegan como mensajes internos y la TUI los pinta
como `sys:`, el mismo cajón donde caen la reinyección del cierre de ODD, el aviso de compactación y cualquier otro aviso
del sistema. Se sabe que vienen del job N solo porque el texto empieza por `[Job N …]`. La regla del proyecto es que **toda
llamada y todo aviso es visible y su actor, distinguible** (la conversación es el diario).

Hoy la TUI ya identifica estos avisos parseando el texto (`parse_notice` en `app/activity.rs`). Funciona, pero depende de
que nadie cambie la redacción de `subagents.rs`, y un mensaje reabierto de una sesión antigua no tiene más pista que el texto.

## Decisión de la persona usuaria (2026-11-07)

Se hace **bien**: el origen viaja en el propio mensaje (opción C), no se detecta por el texto.

## Intención

1. `MessageOrigin` gana una variante `Job(String)` con el id del job que emite el aviso. Los avisos de vigilancia y de
   job terminado nacen con ese origen.
2. `cloth` gana un tipo de línea propio para el actor job, con su prefijo, su token de estilo y su comportamiento en los seis
   estilos de carril (`tree`, `dots`, `bar`, `box`, `tint`, `none`) y en todos los sitios que enumeran los tipos de línea.
3. La TUI pinta el aviso con ese tipo de línea tanto en vivo como al reabrir la sesión. El texto que ve el modelo **no
   cambia** (sigue siendo `[Job N watch] …`).
4. Compatibilidad: los mensajes antiguos (origen `Internal`) se siguen reconociendo por el texto como hasta ahora y se
   pintan igual que antes, o con el actor job si `parse_notice` los reconoce como aviso de job.

## Qué no cambia (A2A)

- Un job **no es un agente A2A** ni una tarea A2A: es un proceso de Loomcoding. Se rotula como actor propio de Loom y **no
  se inventa ni se toca ningún estado A2A**.
- El origen no sale del proceso hacia el proveedor del modelo (el rol sigue siendo `user`) ni hacia ninguna tarjeta o
  mensaje A2A: se comprueba con una prueba.
- Los avisos de subagentes no cambian (siguen su camino actual).

## Riesgos

- **Persistencia y cable:** el origen se guarda con el mensaje y viaja entre daemon y cliente. Una sesión escrita con el
  binario nuevo, abierta con uno antiguo, fallaría al leer la variante nueva. Mismo binario para ambos hoy; se anota en el
  CHANGELOG. Los mensajes antiguos siguen leyéndose (prueba).
- `MessageOrigin` deja de ser `Copy` por llevar un `String`: puede propagar cambios menores por el código que lo copiaba.
- Las exhaustividades de `LineKind` obligan a tocar todos los `match`: es lo buscado, que no quede un sitio sin decidir.

## Preguntas abiertas

1. ¿Glifo y color definitivos del actor? (Se propone un glifo que no choque con `⚙` de herramientas ni `◆` de actividad, y
   un token que reutilice un color ya existente del tema; ajustable.)
2. ¿Se agrupan varios avisos consecutivos del mismo job bajo una sola cabecera? (No en esta spec: cada aviso es una línea.)
