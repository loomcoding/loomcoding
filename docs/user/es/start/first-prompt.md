# Enviar tu primer prompt

**Qué es**

Un prompt es un mensaje que escribes en el compositor. Esta página envía uno en un proyecto que ya tiene un proveedor funcionando.

**Para qué**

- Comprobar que el proveedor responde.
- Aprender las teclas y los comandos que necesitas en la primera sesión.

**Cómo**

1. Arranca `loom` en la carpeta de tu proyecto.
2. Escribe una pregunta corta sobre el proyecto, por ejemplo `¿Qué contiene esta carpeta?`.
3. Pulsa Enter.

## Ejemplo

```text
> ¿Qué contiene esta carpeta? Enumera los ficheros principales.
```

## Teclas y comandos

| Tecla o comando | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|
| Enter | No aplica | No | Envía el prompt. |
| Ctrl+J | No aplica | No | Inserta un salto de línea en el compositor. |
| `/help` | No aplica | No | Abre los atajos de teclado. |
| `/cancel` | No aplica | No | Cancela el turno activo y pausa la cola de prompts pendientes. |
| `/new` | No aplica | No | Inicia una sesión nueva. Solo cuando el agente está inactivo. |
| `/sessions` | No aplica | No | Cambia a otra sesión. Solo cuando el agente está inactivo. |

## Resultado

- Cuando el proveedor responde, la respuesta aparece en la conversación.
- Cuando el agente pide ejecutar una herramienta, aparece un diálogo de permisos. Contéstalo con las teclas que muestra el diálogo.
- Si la respuesta dice `No API key configured`, vuelve a [Conectar un proveedor en la nube](cloud-provider.md).
- Si el proveedor aparece como `down`, ve a [Usar un servidor de modelos local](local-provider.md).

## Siguientes pasos

- [Empezar aquí](index.md)
- [Instalar Loomcoding](install.md)
