#!/usr/bin/env bash
# Ejecuta los cuatro checks del contrato del proyecto, en orden, y para en el
# primero que falle. Uso:
#
#   ./scripts/checks.sh
#   ./scripts/checks.sh --target-dir .verify-target
#
# El parámetro --target-dir sirve para compilar en un directorio aparte y no
# chocar con un daemon en marcha que tenga tomado `target/`.

set -euo pipefail

# La raíz del repo es el padre del directorio de este script, para que funcione
# igual invocado desde la raíz que desde cualquier subdirectorio.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TARGET_ARGS=()
if [[ "${1:-}" == "--target-dir" ]]; then
    if [[ -z "${2:-}" ]]; then
        echo "Falta el valor de --target-dir" >&2
        exit 2
    fi
    TARGET_ARGS=("--target-dir" "$2")
fi

run_check() {
    local name="$1"
    shift
    echo
    echo "==> ${name}: $*"
    if ! "$@"; then
        local code=$?
        echo
        echo "FALLO en '${name}' (exit ${code})." >&2
        echo "Reproduce con: $*" >&2
        exit "$code"
    fi
}

run_check "build"  cargo build "${TARGET_ARGS[@]}"
run_check "test"   cargo test "${TARGET_ARGS[@]}"
run_check "fmt"    cargo fmt --check
run_check "clippy" cargo clippy "${TARGET_ARGS[@]}" -- -D warnings

echo
echo "Los cuatro checks pasan."
