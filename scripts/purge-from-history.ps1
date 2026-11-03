# Quita un fichero de todo el historial local de git, sin reescribir nada de
# `origin` (los commits del remoto no se tocan: el filtro solo recorre
# `origin/master..HEAD`). Uso:
#
#   .\scripts\purge-from-history.ps1 -Path grep_out.txt
#   .\scripts\purge-from-history.ps1 -Path grep_out.txt -DryRun
#
# Por qué existe: el commit 0a67d86 metió `grep_out.txt` de 2,3 GB. Compartido en
# un blob suelto ocupa 15,6 MB, pero el fichero en el árbol ocupa 2,3 GB y viaja
# en el clon de cualquiera que lo haga. No es código ni documentación: no pinta
# nada en el repositorio.
#
# Qué hace, en este orden:
#   1. Comprueba que no hay cambios sin commitear (la reescritura los perdería).
#   2. Guarda la cabeza actual como respaldo (`refs/backup/pre-purge`).
#   3. Reescribe SOLO `origin/master..HEAD` quitando la ruta indicada.
#   4. Poda los objetos huérfanos (`git gc --prune=now`) para recuperar el espacio.
#   5. Verifica que la ruta ya no aparece en ningún commit alcanzable.
#
# Es irreversible en la práctica: el respaldo vive en `.git/refs/backup/`, no en
# otra máquina. No lo ejecutes con commits ajenos sin avisar.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    # Muestra lo que haría y sale, sin tocar el repositorio.
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Push-Location $Root
try {
    if (-not (git rev-parse --git-dir)) {
        throw "no parece un repositorio git: $Root"
    }

    $dirty = git status --porcelain
    if ($dirty) {
        throw "hay cambios sin commitear; haz commit o stash antes de reescribir el historial"
    }

    $upstream = "origin/master"
    $head = git rev-parse HEAD
    $revList = (git rev-list "$upstream..HEAD").Count
    if ($revList -eq 0) {
        Write-Host "nada que reescribir: $upstream..HEAD está vacío." -ForegroundColor Yellow
    }

    $inHistory = git log --oneline --all -- $Path
    if (-not $inHistory) {
        Write-Host "'$Path' no aparece en ningún commit; no hay nada que purgar." -ForegroundColor Yellow
        exit 0
    }

    Write-Host "Se van a reescribir $revList commits ($upstream..HEAD):" -ForegroundColor Cyan
    Write-Host "  respaldo:  refs/backup/pre-purge -> $head"
    Write-Host "  a purgar:  $Path"
    Write-Host "  commits donde aparece:"
    $inHistory | ForEach-Object { Write-Host "    $_" }

    if ($DryRun) {
        Write-Host ""
        Write-Host "DryRun: no se ha tocado nada." -ForegroundColor Yellow
        exit 0
    }

    git update-ref refs/backup/pre-purge $head

    # `filter-branch` está desaconsejado para el historial completo, pero aquí se
    # limita a un rango corto y ya respaldado: no requiere instalar filter-repo.
    $env:FILTER_BRANCH_SQUELCH_WARNING = "1"
    git filter-branch --force --index-filter "git rm --cached --ignore-unmatch -- '$Path'" --prune-empty "$upstream..HEAD"
    if ($LASTEXITCODE -ne 0) {
        throw "filter-branch falló (exit $LASTEXITCODE); recupera con: git reset --hard refs/backup/pre-purge"
    }

    # filter-branch deja sus copias en refs/original; si no se borran, los blobs
    # siguen alcanzables y `gc` no los poda.
    git for-each-ref --format="delete %(refname)" refs/original | git update-ref --stdin
    git reflog expire --expire=now --all
    git gc --prune=now --aggressive

    $still = git log --oneline --all -- $Path
    if ($still) {
        throw "la ruta sigue en el historial: $still"
    }
    if ((git rev-parse "HEAD^{tree}") -ne (git rev-parse "refs/backup/pre-purge^{tree}")) {
        # El árbol de HEAD pierde la ruta purgada; es el único cambio esperado.
        Write-Host "nota: el árbol de HEAD cambió solo por la ruta purgada." -ForegroundColor Yellow
    }
    if ((git ls-tree -r --name-only HEAD | Select-String -SimpleMatch $Path)) {
        throw "la ruta sigue en el árbol de HEAD"
    }

    Write-Host ""
    Write-Host "Purga terminada. Respaldo en refs/backup/pre-purge ($head)." -ForegroundColor Green
    Write-Host "Bórralo cuando estés conforme: git update-ref -d refs/backup/pre-purge"
}
finally {
    Pop-Location
}
