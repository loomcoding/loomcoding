# Ejecuta los cuatro checks del contrato del proyecto, en orden, y para en el
# primero que falle. Uso:
#
#   .\scripts\checks.ps1
#   .\scripts\checks.ps1 -TargetDir .verify-target
#
# El parámetro -TargetDir sirve para compilar en un directorio aparte y no
# chocar con un daemon en marcha que tenga tomado `target/`.

[CmdletBinding()]
param(
    [string]$TargetDir = ""
)

$ErrorActionPreference = "Stop"

# La raíz del repo es el padre del directorio de este script, para que funcione
# igual invocado desde la raíz que desde cualquier subdirectorio.
$Root = Split-Path -Parent $PSScriptRoot
Push-Location $Root
try {
    $targetArgs = @()
    if ($TargetDir -ne "") {
        $targetArgs = @("--target-dir", $TargetDir)
    }

    # Cada entrada: nombre legible, comando y argumentos.
    $checks = @(
        @{ Name = "build";  Cmd = "cargo"; Args = @("build") + $targetArgs },
        @{ Name = "test";   Cmd = "cargo"; Args = @("test") + $targetArgs },
        @{ Name = "fmt";    Cmd = "cargo"; Args = @("fmt", "--check") },
        @{ Name = "clippy"; Cmd = "cargo"; Args = @("clippy") + $targetArgs + @("--", "-D", "warnings") }
    )

    foreach ($check in $checks) {
        $line = "$($check.Cmd) $($check.Args -join ' ')"
        Write-Host ""
        Write-Host "==> $($check.Name): $line" -ForegroundColor Cyan

        & $check.Cmd @($check.Args)
        $code = $LASTEXITCODE

        if ($code -ne 0) {
            Write-Host ""
            Write-Host "FALLO en '$($check.Name)' (exit $code)." -ForegroundColor Red
            Write-Host "Reproduce con: $line" -ForegroundColor Yellow
            exit $code
        }
    }

    Write-Host ""
    Write-Host "Los cuatro checks pasan." -ForegroundColor Green
    exit 0
}
finally {
    Pop-Location
}
