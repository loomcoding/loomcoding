# Especificación: coste informativo de MCP por agente

Estado: implementada; build, formato y clippy pasan. La suite del workspace
conserva dos fallos fuera de este alcance en el composer y el menú slash.

## Objetivo

Informar cuántos tokens de entrada y qué coste estimado aportan las definiciones
de herramientas MCP que se envían al modelo, desglosado por subagente y servidor.
Es información de observabilidad: no cambia presupuestos, límites ni el total
facturado que informa el proveedor.

## Contrato de medición

- Medir las definiciones MCP presentes en cada `ProviderRequest`: nombre,
  descripción y esquema de entrada. El servidor MCP no consume tokens por
  arrancar; los consume el texto de sus herramientas cuando se incluye en una
  petición al modelo.
- Contar el componente en cada petición que vuelva a incluirlo. Presentar el
  estimado por petición y el acumulado de sesión, agrupado por agente y servidor.
- Usar el estimador de tokens existente y etiquetar el resultado como estimado;
  no fingir atribución exacta a partir del uso agregado del proveedor.
- Estimar USD con el precio de entrada del modelo de esa petición. Si no hay
  precio, mostrar los tokens y coste `desconocido`, nunca `$0`.
- Mostrar el desglose separado del uso total/provider-reported. No sumarlo al
  total de tokens o coste ni utilizarlo para decisiones de presupuesto.
- Usar el nombre namespaced de cada tool para atribuirla a su servidor MCP y la
  sesión del agente local para atribuirla al subagente correspondiente.

## Skills

Los fragmentos locales cargados desde `scope.skills` también se atribuyen por
petición con el mismo estimador y etiqueta informativa. Las skills de tarjeta
siguen siendo solo metadatos y no cargan archivos.

## Presentación

El widget MCP lista cada servidor con su estimación de tokens por petición y su
acumulado de sesión; el detalle puede desplazarse. El desglose se asocia al
agente que envió la petición, no se duplica en todos los agentes del registro.
La ausencia de llamadas MCP se distingue de datos todavía no observados.

## Límites y privacidad

- Solo cuentan definiciones de herramientas MCP que realmente se entregaron al
  provider; herramientas configuradas pero no disponibles para ese agente no
  generan coste estimado.
- No registrar cuerpos de herramientas ni argumentos de usuario como coste de
  definiciones.
- No realizar llamadas de red para consultar facturación.
- No implementar el runtime local de skills en esta tarea.

## Aceptación

1. Dos agentes con listas MCP distintas muestran estimaciones distintas y
   atribuibles a sus servidores.
2. La misma definición se cuenta de nuevo en cada petición que la incluya.
3. La suma es visible por servidor y agente y se recupera con la sesión si el
   ledger ya se persiste junto al uso de provider.
4. Coste/token estimado aparece separado del uso agregado informado por el
   provider y no cambia presupuesto ni cifra total.
5. Sin precio se muestra coste desconocido; sin petición MCP se muestra cero
   peticiones observadas, sin afirmar coste de arranque.
6. Las skills no aparecen como cargadas salvo que el loader local las haya leído.
7. La verificación usa providers falsos/locales, sin tráfico real.
