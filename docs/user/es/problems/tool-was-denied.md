# ¿Por qué denegaron mi herramienta, y cómo la permito?

**Qué es**

Las herramientas que cambian ficheros o ejecutan comandos de shell piden tu permiso antes. Una herramienta que deniegas no se ejecuta. Esta página explica el diálogo y los modos que responden sin preguntar.

**Para qué**

- Permitir una acción concreta, o un tipo de acción para la sesión.
- Entender por qué una ejecución con `--print` deniega todas las peticiones.

**Cómo**

1. Lee el diálogo. Muestra la herramienta y su entrada.
2. Pulsa `a` para permitir una vez, `s` para permitir en la sesión, o `d` o `Esc` para denegar.

## Ejemplo

```text
Dialog: bash
  command: cargo test
  [Permitir]  [Permitir en la sesión]  [Denegar]
```

`Permitir en la sesión` recuerda el par de herramienta y acción, por ejemplo `bash` y `run`. No recuerda un comando exacto.

## Mensajes y soluciones

| Síntoma | Causa | Solución |
|---|---|---|
| Denegaste una herramienta por error | Una herramienta denegada no se ejecuta. | Pide al agente que la ejecute de nuevo. El diálogo vuelve a abrirse. |
| `loom --print` se ejecutó, pero las herramientas no | En modo impresión se deniega cada petición, salvo que añadas `--yolo`. | Ejecuta `loom` sin `--print` y responde al diálogo. Usa `--yolo` solo en un proyecto de confianza. |
| Un subagente espera en la fase «awaiting permission» | Un subagente que necesita permiso espera tu respuesta. | Responde a su petición. La cabecera del flock muestra la fase. |
| Un agente del proyecto no se ejecuta | Los agentes definidos en la carpeta del proyecto necesitan tu aprobación antes de ejecutarse. | Aprueba el agente cuando se te pida. Para aprobar todos los agentes del proyecto, consulta el ajuste de abajo. |

## Modos de arranque

| Modo | Cómo se arranca | Qué pasa con una petición de permiso |
|---|---|---|
| Interactivo | `loom` | Se abre el diálogo y decides tú. |
| Aprobación automática | `loom --yolo` | Cada petición se aprueba sin preguntar. Úsalo solo en proyectos de confianza. |
| Impresión | `loom --print "<prompt>"` | Cada petición se deniega, salvo que añadas también `--yolo`. |

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `--yolo` (opción de arranque) | Desactivada | No aplica: afecta solo al arranque en el que la usas |
| `[agents] trust_project_agents` en `loomcoding.toml` | `false` | Sí |

## Resultado

- Una herramienta permitida se ejecuta. Una herramienta denegada no se ejecuta.
- Si no estás seguro de lo que hace un comando, deniégalo y pide al agente que te lo explique antes.

## Siguientes pasos

- [Permisos](../concepts/permissions.md)
- [Agentes, subagentes y jobs](../concepts/agent-subagents-jobs.md)
- [Configuración](../reference/configuration.md)
