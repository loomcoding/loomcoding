# ¿Cómo funcionan las sesiones y dónde se guardan?

**Qué es**

Una sesión es el registro de una conversación: tus mensajes, las respuestas del agente, las llamadas a herramientas y los subagentes. Loomcoding guarda cada mensaje en una base de datos a medida que ocurre.

**Para qué**

- Volver a una conversación anterior del mismo proyecto.
- Empezar una conversación limpia sin perder la antigua.
- Volver desde un subagente a la sesión que lo lanzó.

**Cómo**

1. Escribe `/new` para empezar una sesión vacía.
2. Escribe `/sessions` para elegir una sesión guardada de la lista.
3. Escribe `/parent` para volver de un subagente a la sesión padre.

## Ejemplo

```text
> /new          empieza una sesión vacía
> /sessions     cambia a una sesión guardada
> /focus <id>   envía tus siguientes mensajes a un subagente
> /parent       vuelve a la sesión que lo lanzó
```

## Dónde están los datos

| Qué | Dónde |
|---|---|
| Sesiones, mensajes y subagentes | `<inicio>/.loomcoding/sessions.db` |
| Ajustes del proyecto | `loomcoding.toml`, en la carpeta del proyecto |

`<inicio>` es tu carpeta personal, o la carpeta indicada en `LOOMCODING_HOME` cuando esa variable está definida y no está vacía. Para probar Loom sin tocar tus datos reales, apunta `LOOMCODING_HOME` a una carpeta temporal vacía.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `LOOMCODING_HOME` (variable de entorno) | Sin valor: tu carpeta personal | Sí. Defínela antes de arrancar Loom. |

## Resultado

`/new` y `/sessions` solo funcionan cuando el agente está inactivo. Si hay un turno en curso, espera a que termine o cancélalo con `/cancel`.

## Siguientes pasos

- [Agentes, subagentes y jobs](agent-subagents-jobs.md)
- [Contexto y compactación](context-and-compaction.md)
