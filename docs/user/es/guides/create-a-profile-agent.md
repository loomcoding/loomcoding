# Crear un agente de perfil en Markdown

**Qué es**

Un agente de perfil es un fichero Markdown llamado `<nombre>.md`. El frontmatter del principio fija su nombre y sus opciones. El cuerpo contiene las instrucciones que sigue el agente.

**Para qué**

- Dar a un subagente una especialidad, como revisar cambios.
- Compartir un mismo agente entre todos tus proyectos, o guardarlo en un solo repositorio.
- Ejecutarlo con `/agent <nombre> <tarea>`.

**Cómo**

1. Crea `<nombre>.md` en la carpeta de agentes: `~/.agents/agents/` (global) o `.agents/agents/` dentro del proyecto.
2. Escribe el frontmatter, con `name` (igual que el nombre del fichero) y `description`.
3. Escribe las instrucciones en el cuerpo y guarda. Loomcoding vuelve a leer la carpeta mientras se ejecuta.

## Ejemplo

```md
---
name: reviewer
description: Revisa un cambio en busca de errores y de pruebas que falten.
---

Revisas los cambios que te entregan.
Primero indica los errores y después las pruebas que faltan.
```

```text
> /agents
> /agent reviewer revisa el último cambio en src/parser.rs
```

## Dónde viven las carpetas de agentes

| Nivel | Carpeta por defecto | Ajuste | Aprobación |
|---|---|---|---|
| Global | `~/.agents/agents/` | `[agents] global_dir` | No hace falta |
| Proyecto | `.agents/agents/` en la raíz del proyecto | `[agents] project_dir` | Necesaria, en `/agents` |

## Campos

| Campo | Obligatorio | Qué es |
|---|---|---|
| `name` | Sí | Debe ser igual al nombre del fichero. Hasta 64 caracteres: letras minúsculas, dígitos y guiones, sin guion al principio ni al final. |
| `description` | Sí | Texto breve que describe el agente. |
| `model` | No | Identificador del modelo de este agente. |
| `provider` | No | Bloque de proveedor propio: `kind`, `base_url` y `api_key_env`. |
| `effort` | No | Nivel de esfuerzo de razonamiento. |
| `max_turns` | No | Límite de turnos del agente. |
| `max_depth` | No | Cuántos niveles de subagentes puede lanzar. |
| `persona` | No | `role`, `goal` y `backstory`, opcionales. |

Los campos desconocidos se ignoran y se muestran como aviso. Los campos `scope` y `delegation` se leen, pero todavía no se aplican.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[agents] global_dir` | `~/.agents/agents` | Sí |
| `[agents] project_dir` | `.agents/agents` en el proyecto | Sí |
| `[agents] trust_project_agents` | `false` | Sí |

Loomcoding lee la sección `[agents]` al arrancar. Los ficheros de perfil no necesitan reinicio: los cambios en las carpetas se recogen mientras Loomcoding se ejecuta.

## Aprobar un agente de proyecto

Los agentes de proyecto esperan tu aprobación antes de ejecutarse.

1. Abre `/agents`. Un agente de proyecto nuevo muestra `awaiting approval`.
2. Selecciónalo y pulsa `a` para aprobarlo, o `r` para revocarlo.
3. Si el fichero cambia después de aprobarlo, la aprobación deja de valer hasta que lo apruebes de nuevo.

Pon `trust_project_agents = true` para aprobar todos los agentes de proyecto sin preguntar. Úsalo solo en repositorios en los que confíes.

## Resultado

`/agents` lista el agente, y `/agent <nombre> <tarea>` lo ejecuta.

## Si no funciona

| Síntoma | Solución |
|---|---|
| El agente no aparece. | Comprueba que `name` es igual al nombre del fichero y que el frontmatter empieza y termina con líneas `---`. |
| Un aviso señala un campo desconocido. | Revisa la ortografía. Los campos desconocidos se ignoran. |
| Un agente de proyecto sigue en `awaiting approval`. | Apruébalo en `/agents`. |
| Un agente de proyecto aprobado dejó de funcionar. | El fichero cambió. Revísalo y apruébalo de nuevo. |

## Siguientes pasos

- [Agentes, subagentes y jobs](../concepts/agent-subagents-jobs.md)
- [Permisos](../concepts/permissions.md)
- [Usar un agente A2A remoto](use-remote-a2a-agents.md)
