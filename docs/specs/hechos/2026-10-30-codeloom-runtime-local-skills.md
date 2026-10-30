# Especificación: runtime local acotado de skills

Estado: **hecho**. Verificado el 2026-10-30: `cargo fmt --check`, `clippy -D warnings`, `build` y `test` completos en verde con `CARGO_TARGET_DIR=target/e2e`.

## Objetivo

Hacer que un agente local pueda cargar el contenido de las skills mencionadas
explícitamente en su `scope.skills` e incluirlo en el contexto del modelo. Debe
medirse su consumo de tokens de forma informativa. La lista `AgentCard.skills`
continúa siendo metadato público y no carga archivos.

## Alcance inicial

- Consumir únicamente archivos Markdown `SKILL.md`; no ejecutar scripts,
  instalar dependencias ni registrar tools desde una skill.
- Aplicar el scope solo a agentes locales. Una lista vacía significa ninguna
  skill; no activar todas por defecto.
- Cargar skills durante la creación/configuración del agente y mantener el
  conjunto estable durante la sesión. Sin hot-reload en esta primera versión.
- Incluir el fragmento limitado en el prompt y contar sus tokens por petición
  con el estimador existente, etiquetados como estimación informativa.
- Indicar por agente qué IDs se cargaron, cuáles faltan y cuántos tokens estima
  cada fragmento.

## Resolución y seguridad

- Raíces: `.agents/skills/<id>/SKILL.md` en el proyecto y
  `~/.agents/skills/<id>/SKILL.md` en el perfil de usuario. Prevalece la skill
  del proyecto si el ID existe en ambas raíces.
- Tratar cada ID como un único segmento seguro; rechazar rutas absolutas,
  separadores, `..`, enlaces que escapen de la raíz y archivos no regulares.
- Leer solo las skills enumeradas en `scope.skills`; mantener aislamiento entre
  agentes y no transmitir contenido a tarjetas A2A ni a agentes remotos.
- Aplicar topes de 16 skills, 4 KiB por archivo, 2 KiB de frontmatter y 6 KiB
  de texto total; informar rechazos en el estado por skill.
- Los errores de una skill no impiden cargar las otras. No revelar rutas
  absolutas o contenido de skills en logs innecesarios.

## Tokens y coste

- Los tokens del fragmento se repiten en cada petición que lo incluya; el
  coste se estima por petición usando el precio de entrada del modelo, si existe.
- Mostrarlo separado del total agregado del provider y del presupuesto. No
  afirmar que es uso exacto ni sumar dos veces el desglose al total.
- El contenido `SKILL.md` realmente incluido es atribuible; el nombre de la
  skill y las declaraciones sin archivo cargado no se cuentan como contenido.

## Fuera de alcance

- Selector automático/clasificador, puerta de descubrimiento y activación por
  turno.
- `AgentCard.skills` como loader, soporte de skill remota, MCP hot-reload,
  vigilancia de archivos y actualización a mitad de sesión.
- Ejecutar scripts, exponer tools desde skills o conceder permisos por cargarla.
- Cambios a presupuestos o al total de uso informado por el provider.

## Aceptación

1. Un agente local solo recibe los fragmentos enumerados en su `scope.skills`.
2. Agentes con scopes distintos no comparten contenido ni métricas.
3. IDs faltantes, inválidos o que escapan de la raíz fallan de forma acotada y
   visible; no se lee fuera de las raíces aprobadas.
4. Los límites por archivo y prompt se aplican y se informan.
5. El widget distingue declarada, encontrada, cargada y error; muestra tokens
   estimados y coste informativo cuando hay precio.
6. Los fragmentos solo se cuentan en las peticiones donde se incluyeron; no se
   alteran presupuesto ni total de uso/provider-reported.
7. Tests con directorios temporales y provider falso; sin red ni ejecución de
   contenido de skill.

## Decisiones

- Solo perfiles de agentes locales cargan sus propios `scope.skills`; no se
  heredan ni se consultan para agentes remotos.
- El loader lee Markdown como datos, no ejecuta scripts y reporta cada estado.
- Los tokens se estiman por fragmento cargado y se atribuyen solo a las
  peticiones que lo incluyen; el precio es informativo y separado del total.
