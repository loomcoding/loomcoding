# Atajos de teclado

**Qué es**

Las teclas que Loomcoding atiende en el compositor, en la transcripción, en la pantalla global y en los paneles. Esta página recoge solo las teclas que el código atiende hoy. Una tecla que no aparece aquí no está documentada.

**Para qué**

- Encontrar la tecla de una acción sin abrir el panel de ayuda.
- Saber qué tecla funciona en cada zona.
- Comprobar la diferencia entre el panel de ayuda y el código, al final de la página.

**Cómo**

1. Escribe en el compositor. Tab mueve el foco entre el compositor y la transcripción.
2. Las teclas globales funcionan en todas las zonas, salvo que haya un panel o un diálogo abierto.
3. Los paneles usan sus propias teclas mientras están abiertos. Esc, o la tecla que indica el título del panel, lo cierra.

## Ejemplo

```text
Compositor vacío:  ?        abre el panel de ayuda
Compositor:        Ctrl+J   salto de línea;  Enter envía la petición
Transcripción:     Tab, luego j y k desplazan; y copia el bloque activo
```

## Compositor

| Tecla | Qué hace |
|---|---|
| Enter | Envía la petición. `/agent` y `@nombre` se leen al enviar. |
| Ctrl+J | Inserta un salto de línea. |
| Tab | Completa el nombre de un agente si el compositor lo contiene. Si no, cambia el foco. |
| Shift+Tab | Cambia el foco hacia atrás. |
| Shift+Izquierda, Shift+Derecha, Shift+Arriba, Shift+Abajo, Shift+Inicio, Shift+Fin | Amplía la selección de texto en el compositor. |
| Ctrl+C | Con selección, la copia y sigue abierto. Sin selección, sale. |
| Ctrl+V | Con el compositor enfocado, pega el portapapeles. Una imagen se adjunta; si no, se pega el texto. |
| Retroceso | Con el compositor vacío, quita el último adjunto de imagen. |
| Supr | Borra el carácter que sigue al cursor. |
| RePág, AvPág | Desplaza la transcripción 10 líneas. |
| ? | Con el compositor vacío y sin panel abierto, abre el panel de ayuda. En otro caso es un carácter. |

## Transcripción

Pulsa Tab hasta que la transcripción tenga el foco.

| Tecla | Qué hace |
|---|---|
| Arriba o k, Abajo o j | Desplaza una línea. Con el aspecto Crush (cromo plano), se mueve entre bloques. |
| u, d | Media página arriba, media página abajo. |
| RePág o b, AvPág o f | Una página arriba, una página abajo. |
| Espacio | Pliega o despliega el bloque largo activo. Si no hay ninguno, avanza una página. |
| Enter | Pliega o despliega el bloque largo activo, o abre o cierra un nodo de subagente. |
| g o Inicio, G o Fin | Va al principio, o sigue el final. |
| [ y ] | Va al bloque anterior o al siguiente. |
| y o c | Copia el bloque activo. |
| v | Selecciona la última línea. Después, j y k amplían la selección, y o c la copian, y v la borra. |
| Izquierda, Derecha | En un nodo de subagente, lo pliega o lo despliega. |

## Teclas globales

| Tecla | Qué hace |
|---|---|
| F1 | Abre el panel de ayuda. Esc o F1 lo cierra. |
| F2 | Abre los ajustes del proveedor. |
| F3 | Abre el selector de sesiones. No está disponible mientras corre un turno. |
| Ctrl+T | Abre el panel de las últimas llamadas a herramientas. |
| Ctrl+K | Abre el panel de peticiones pendientes (la cola). |
| Ctrl+B | Muestra u oculta la barra lateral. |
| Ctrl+O | Expande o contrae la última salida de herramienta. |
| Ctrl+G | Recorre el destino del compositor: esta sesión, o un subagente vivo. |
| Ctrl+X | Parada de emergencia: cancela el turno, los subagentes y los jobs. |
| Alt+Izquierda | Vuelve a la sesión padre. |
| Ctrl+Q | Sale. |
| Esc | Borra una selección de texto o del compositor. Si no, cierra la cola, devuelve el foco de un panel lateral al compositor, o cancela el turno activo. Púlsala dos veces en menos de 350 ms para salir. |

## Paneles

| Panel | Teclas |
|---|---|
| Cola (Ctrl+K) | Arriba y Abajo seleccionan. Enter edita la petición seleccionada. Ctrl+Arriba y Ctrl+Abajo la mueven. Supr la quita. Espacio pausa o reanuda la cola. Ctrl+X pausa la cola y cancela el turno, mientras corre un turno. Esc la cierra. |
| Llamadas a herramientas (Ctrl+T) | Arriba y Abajo seleccionan. RePág y AvPág desplazan. Izquierda y Derecha desplazan a los lados. Inicio y Fin saltan. y copia la salida. Esc o Ctrl+T lo cierra. |
| Selector de sesiones (F3) | Arriba y Abajo, o j y k, mueven. Enter abre. n inicia una sesión nueva. Ctrl+F activa o desactiva la búsqueda. Esc sale primero de la búsqueda; después cierra el selector si se abrió desde un chat, y si no, sale. |
| A2A (`/a2a`) | Tab o Derecha cambian el foco. Arriba y Abajo mueven. RePág y AvPág desplazan el detalle. Inicio va al principio. Esc o q lo cierra. |
| Agentes (`/agents`) | Arriba y Abajo mueven. Tab y Shift+Tab cambian el foco. RePág y AvPág saltan de 5 en 5. a aprueba y r rechaza el agente seleccionado. e lo edita. / filtra por texto, t por etiqueta, x limpia los filtros. Esc o q lo cierra. |
| Ayuda (F1) | F1, ? o Esc lo cierra. |

## Diferencias con el panel de ayuda

El panel de ayuda (F1) dice que Enter y Ctrl+J envían la petición. En el código, Enter envía la petición y Ctrl+J inserta un salto de línea. Esta página sigue al código.
