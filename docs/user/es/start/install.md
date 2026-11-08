# Instalar Loomcoding desde el código fuente

**Qué es**

Loomcoding se compila a partir del código Rust de este repositorio. Esta página compila el binario `loom` y comprueba que arranca.

**Para qué**

- Tener un binario `loom` en tu máquina.
- Comprobar la versión antes de configurar nada.

**Cómo**

1. Instala el toolchain de Rust con rustup.
2. Desde la raíz del repositorio, ejecuta `cargo build`.
3. Ejecuta el binario con `--version`.

## Requisitos

| Requisito | Valor | Dónde se fija |
|---|---|---|
| Toolchain de Rust | `stable`, con `rustfmt` y `clippy` | `rust-toolchain.toml` |
| Binarios compilados | `loom` (nombre corto) y `loomcoding`, el mismo programa | `crates/loomcoding-cli` |
| Plataforma comprobada para esta guía | Windows | El almacenamiento seguro de claves funciona solo en Windows |

## Ejemplo

PowerShell de Windows, con una carpeta vacía como perfil:

```text
PS> cargo build
PS> New-Item -ItemType Directory -Path <carpeta-vacia>
PS> $env:LOOMCODING_HOME = "<carpeta-vacia>"
PS> target\debug\loom.exe --version
loom 0.1.0
```

Usa una carpeta vacía en `LOOMCODING_HOME` mientras pruebas. Si no, Loomcoding usa tu perfil real.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `LOOMCODING_HOME` (variable de entorno) | Sin definir: la carpeta personal del sistema es el perfil | Sí |
| `cargo build` (comando) | No aplica: compila un binario de depuración en `target/debug/` | No aplica |

## Resultado

- `loom 0.1.0` y código de salida 0 indican que el binario funciona.
- `loom --help` lista `--print`, `--attach` y `--yolo`. Ejecutarlo no crea nada en la carpeta del perfil.
- Si no se encuentra `loom`, usa la ruta completa `target\debug\loom.exe` desde la raíz del repositorio.

## Siguientes pasos

- [Qué pasa en el primer arranque](first-run.md)
