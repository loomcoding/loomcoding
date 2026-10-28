# Coste real y estimado en todos los adapters

## Objetivo

Conservar y mostrar el coste de cada respuesta de modelo desde cualquier adapter. Si la respuesta del proveedor incluye un coste facturado, CodeLoom lo guarda como real; si no, usa los precios congelados del catálogo para estimarlo. Nunca se presenta una estimación como coste real.

## Alcance

- Contrato común `Usage` con coste reportado opcional en USD.
- Lectura del campo `usage.cost` cuando venga en respuestas compatibles con OpenAI y Anthropic; adapters futuros podrán rellenar el mismo campo.
- Registro persistente y migración SQLite aditiva.
- Presupuesto y TUI consumen el coste real cuando existe; si no, el coste del catálogo sigue siendo estimado.
- Mantener tokens, precios históricos y sesiones existentes. Un dato real inválido (negativo o no finito) se descarta.

## Fuera de alcance

Consultar APIs de facturación tras cada respuesta, conversión de monedas, precios dinámicos y añadir costes estimados para modelos sin tarifa catalogada.

## Criterios de aceptación

1. Todo adapter puede reportar coste en el contrato compartido sin cambiar el almacén por proveedor.
2. Coste real y estimado sobreviven reinicio de SQLite y se distinguen en la interfaz.
3. Los presupuestos usan el coste real disponible y conservan la marca de parcial cuando faltan costes.
4. Eventos de uso fragmentados no duplican el coste final reportado.
5. Pruebas de parser y persistencia usan respuestas/DB locales, sin red.

## Decisiones

`Usage.reported_cost_usd` es opcional y representa el importe de la respuesta facturado por el adapter. `UsageRecord.cost()` prefiere ese importe; en su ausencia calcula con tarifas guardadas. SQLite añade una columna nullable, sin reescribir registros previos.
