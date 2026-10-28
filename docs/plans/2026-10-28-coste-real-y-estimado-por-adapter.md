# Plan: coste real y estimado por adapter

1. Añadir campo opcional de coste reportado al contrato Usage, con compatibilidad serde.
2. Extraerlo en parsers OpenAI compatible y Anthropic desde `usage.cost`; validar valores.
3. Acumularlo por respuesta y guardarlo en UsageRecord; hacer migración SQLite nullable y compatibilidad de lecturas.
4. Preferirlo al calcular presupuesto y cambiar la etiqueta TUI para diferenciar coste real, estimado y parcial.
5. Añadir pruebas unitarias de los parsers, cálculo y persistencia; ejecutar los cuatro checks del proyecto.

## Riesgos y límites

Los endpoints directos que no devuelven importe facturado seguirán mostrando estimaciones. Solo se marca como real un importe explícito del proveedor.
