# Quita un fichero de todo el historial local de git, sin reescribir nada de
# `origin` (los commits del remoto no se tocan: el filtro solo recorre
# `origin/master..HEAD`). Uso:
#
#   .\scripts\purge-from-history.ps1 -Path grep_out.txt
#   .\scripts\purge-from-history.ps1 -Path grep_out.txt -DryRun
#   .\scripts\purge-from-history.ps1 -Path grep_out.txt -Repo C:\otro\repo
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
    [switch]$DryRun,

    # Repositorio sobre el que actuar. Por defecto, el del script. La prueba de humo
    # lo apunta a un repositorio desechable.
    [string]$Repo = ""
)

$ErrorActionPreference = "Stop"

$Root = if ($Repo -ne "") { (Resolve-Path $Repo).Path } else { Split-Path -Parent $PSScriptRoot }
Push-Location $Root
try {
    if (-not (git rev-parse --git-dir 2>$null)) {
        throw "no parece un repositorio git: $Root"
    }

    $dirty = @(git status --porcelain)
    if ($dirty.Count -gt 0) {
        throw "hay cambios sin commitear; haz commit o stash antes de reescribir el historial"
    }

    $upstream = "origin/master"
    $head = git rev-parse HEAD
    $revList = (git rev-list "$upstream..HEAD").Count
    if ($revList -eq 0) {
        Write-Host "nada que reescribir: $upstream..HEAD está vacío." -ForegroundColor Yellow
    }

    $inHistory = @(git log --oneline HEAD -- $Path)
    if ($inHistory.Count -eq 0) {
        Write-Host "'$Path' no aparece en los commits de HEAD; no hay nada que purgar." -ForegroundColor Yellow
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

    # Un segundo intento tiene que partir del estado limpio: si quedan copias de
    # `filter-branch` en refs/original, apuntan al historial con el fichero.
    $originalRefs = @(git for-each-ref --format="%(refname)" refs/original)
    foreach ($ref in $originalRefs) {
        git update-ref -d $ref
    }

    # El respaldo NO se sobrescribe: si ya existe de un intento anterior, se queda
    # como está. Lo que hay que poder deshacer es el trabajo previo a la primera
    # purga, no el de la purga de una purga.
    $hasBackup = @(git for-each-ref --format="%(refname)" refs/backup/pre-purge).Count -gt 0
    if ($hasBackup) {
        $kept = git rev-parse --short refs/backup/pre-purge
        Write-Host "  respaldo ya existente: se conserva refs/backup/pre-purge ($kept)" -ForegroundColor Yellow
    }
    else {
        git update-ref refs/backup/pre-purge $head
    }

    # `filter-branch` está desaconsejado para el historial completo, pero aquí se
    # limita a un rango corto y ya respaldado: no requiere instalar filter-repo.
    $env:FILTER_BRANCH_SQUELCH_WARNING = "1"
    git filter-branch --force --index-filter "git rm --cached --ignore-unmatch -- '$Path'" --prune-empty "$upstream..HEAD"
    if ($LASTEXITCODE -ne 0) {
        throw "filter-branch falló (exit $LASTEXITCODE); recupera con: git reset --hard refs/backup/pre-purge"
    }

    # filter-branch deja sus copias en refs/original; si no se borran, los blobs
    # siguen alcanzables y `gc` no los poda. Se borra ref a ref: pasarle varias
    # líneas a `update-ref --stdin` falla en Windows porque cmd/PS añaden `\r`.
    $originalRefs = @(git for-each-ref --format="%(refname)" refs/original)
    foreach ($ref in $originalRefs) {
        git update-ref -d $ref
    }
    git reflog expire --expire=now --all
    git gc --prune=now --aggressive

    # La comprobación no puede ser `--all`: el respaldo `refs/backup/pre-purge` conserva
    # a propósito el historial anterior (para poder deshacer), así que saldría siempre.
    # Lo que importa es que la ruta desaparezca de lo que se va a publicar: HEAD y las
    # demás refs, contando el respaldo.
    $still = @(git log --oneline HEAD -- $Path)
    if ($still.Count -gt 0) {
        throw "la ruta sigue en el historial: $($still -join '; ')"
    }
    if (@(git ls-tree -r --name-only HEAD | Select-String -SimpleMatch $Path)) {
        throw "la ruta sigue en el árbol de HEAD"
    }

    Write-Host ""
    Write-Host "Purga terminada. Respaldo en refs/backup/pre-purge." -ForegroundColor Green
    Write-Host "Bórralo cuando estés conforme: git update-ref -d refs/backup/pre-purge"
}
finally {
    Pop-Location
}
