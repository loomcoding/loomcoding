# Prueba de humo de scripts/purge-from-history.ps1 en un repositorio desechable.
# Comprueba lo que de verdad importa: que el fichero sale del historial, que los
# commits se conservan y que una segunda ejecucion no rompe nada.

$ErrorActionPreference = "Stop"

$sandbox = Join-Path ([System.IO.Path]::GetTempPath()) ("loom-purge-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $sandbox | Out-Null
$script = (Resolve-Path (Join-Path $PSScriptRoot "..\purge-from-history.ps1")).Path

try {
    Push-Location $sandbox
    try {
        git init -q --initial-branch=master .
        git config user.email "test@example.com"
        git config user.name "test"

        Set-Content -Path "a.txt" -Value "uno"
        git add a.txt
        git commit -q -m "primero"

        # El fichero que hay que purgar, con contenido suficientemente grande.
        $big = ("x" * 1024) + ("y" * 1024)
        Set-Content -Path "grep_out.txt" -Value $big
        Set-Content -Path "b.txt" -Value "dos"
        git add grep_out.txt b.txt
        git commit -q -m "mete el volcado"

        Set-Content -Path "c.txt" -Value "tres"
        git add c.txt
        git commit -q -m "despues del volcado"

        # `origin` simulado: el script solo reescribe origin/master..HEAD.
        $root = (git rev-list --max-parents=0 HEAD)
        git update-ref refs/remotes/origin/master $root

        $before = @(git rev-list --count "origin/master..HEAD")[0]
        $messagesBefore = @(git log --format="%s" "origin/master..HEAD")

        & $script -Path grep_out.txt -Repo $sandbox | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "la purga fallo (exit $LASTEXITCODE)" }

        # `--all` no vale aquí: el respaldo conserva a propósito el historial viejo.
        $still = @(git log --oneline HEAD -- grep_out.txt)
        if ($still.Count -gt 0) { throw "sigue en el historial: $($still -join '; ')" }
        if (@(git ls-tree -r --name-only HEAD | Select-String -SimpleMatch "grep_out.txt").Count -gt 0) {
            throw "sigue en el arbol de HEAD"
        }
        if (@(git for-each-ref --format="%(refname)" refs/original).Count -gt 0) {
            throw "quedan refs/original y el blob no se podra podar"
        }

        $after = @(git rev-list --count "origin/master..HEAD")[0]
        if ($after -ne $before) {
            throw "cambiaron commits sin purgar: $before -> $after"
        }
        $messagesAfter = @(git log --format="%s" "origin/master..HEAD")
        if (($messagesBefore -join "|") -ne ($messagesAfter -join "|")) {
            throw "cambiaron los mensajes de commit"
        }

        # Los otros ficheros siguen intactos y el arbol se puede leer.
        if (-not (Test-Path "b.txt")) { throw "b.txt desaparecio del arbol de trabajo" }
        git fsck --no-progress --strict 2>&1 | Out-Null

        # Segunda ejecucion: ya no queda nada que purgar y tiene que decirlo sin fallar,
        # conservando el respaldo del primer intento (es lo unico que permite deshacer).
        $backupBefore = (git rev-parse refs/backup/pre-purge)
        & $script -Path grep_out.txt -Repo $sandbox | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "la segunda purga fallo (exit $LASTEXITCODE)" }
        $backupAfter = (git rev-parse refs/backup/pre-purge)
        if ($backupBefore -ne $backupAfter) {
            throw "la segunda ejecucion movio el respaldo: $backupBefore -> $backupAfter"
        }

        Write-Host "OK: purga, conservacion de commits, respaldo intacto y segunda ejecucion." -ForegroundColor Green
        exit 0
    }
    finally {
        Pop-Location
    }
}
finally {
    if (Test-Path $sandbox) {
        # git deja objetos en solo-lectura; se quita el atributo antes de borrar.
        Get-ChildItem -Recurse -Force $sandbox -ErrorAction SilentlyContinue |
            ForEach-Object { $_.Attributes = "Normal" }
        Remove-Item -Recurse -Force $sandbox -ErrorAction SilentlyContinue
    }
}
