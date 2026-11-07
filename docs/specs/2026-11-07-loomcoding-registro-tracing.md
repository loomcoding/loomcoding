# Registro de `tracing` a fichero — spec

## Problema

El código emite eventos `tracing` (destino `odd` para la medición del desarrollo orgánico, destino `output_filter`
para la compresión de salida, y varios `warn!` sueltos), pero **ningún binario instala un suscriptor**. Resultado:
`RUST_LOG` no enseña nada y las cifras que decidirían si activar la compresión por defecto, o si ODD es útil, no se pueden
leer. Es una pérdida de observabilidad, que es el valor central del producto.

## Intención

1. Si `RUST_LOG` está definida, los procesos de Loomcoding (cliente TUI y daemon) instalan un suscriptor que escribe a un
   **fichero** `logs/loomcoding.log` dentro del directorio de datos (`LOOMCODING_HOME` si está, el de siempre si no).
2. Si `RUST_LOG` no está definida, **no se instala nada**: comportamiento y coste idénticos a hoy.
3. Nunca se escribe en stdout ni en stderr: la TUI es dueña de la terminal y un solo byte rompería el dibujo.
4. Cota (decálogo de contexto y coste): el fichero rota al pasar de 5 MiB conservando una copia anterior
   (`loomcoding.log.1`); jamás crece sin límite.
5. Formato de línea legible por humanos y por `grep`: marca de tiempo, nivel, destino y campos.
6. No se registran datos personales de forma nueva: lo que ya se emite (nombres de regla, cifras) es lo que sale.

## Qué no cambia

- Ningún estado ni semántica A2A. No hay dependencias nuevas: `tracing-subscriber` ya está en el `Cargo.toml` del espacio
  de trabajo con la característica `env-filter`. La rotación es propia y mínima (sin `tracing-appender`).
- El filtrado de contenido sensible no es objetivo de esta spec: `RUST_LOG` es una decisión explícita de quien depura.

## Riesgos

- Dos procesos (cliente y daemon) escribiendo al mismo fichero: se resuelve con un fichero por proceso
  (`loomcoding-<rol>.log`, con `<rol>` = `cliente` o `daemon`), cada uno con su rotación.
- Un fallo al abrir el fichero (permisos, disco) nunca debe impedir arrancar: se ignora en silencio.

## Preguntas abiertas

- ¿Ruta por defecto en sistemas con directorio de datos distinto? Se usa el mismo resolvedor que el resto de ficheros
  (`LOOMCODING_HOME`), sin inventar uno nuevo.
