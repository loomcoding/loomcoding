# Comprueba que la reescritura del historial solo cambio el fichero purgado: mismo
# numero de commits, mismos mensajes y mismos arboles salvo la ruta eliminada.
# Uso:
#
#   .\scripts\verify-rewrite.ps1 -Before refs/backup/pre-purge -After master -Path grep_out.txt
#
# Si esto pasa, el respaldo puede borrarse sin miedo.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Before,
    [string]$After = "HEAD",
    [Parameter(Mandatory = $true)][string]$Path
)

$ErrorActionPreference = "Stop"

$beforeCommits = @(git rev-list --reverse $Before)
$afterCommits = @(git rev-list --reverse $After)
if ($beforeCommits.Count -ne $afterCommits.Count) {
    throw "cambia el numero de commits: $($beforeCommits.Count) -> $($afterCommits.Count)"
}

$failures = 0
for ($i = 0; $i -lt $beforeCommits.Count; $i++) {
    $old = $beforeCommits[$i]
    $new = $afterCommits[$i]
    $oldMsg = (git log -1 --format="%s" $old)
    $newMsg = (git log -1 --format="%s" $new)
    if ($oldMsg -ne $newMsg) {
        Write-Host "commit ${i}: mensaje distinto ($oldMsg / $newMsg)" -ForegroundColor Red
        $failures++
        continue
    }

    # Los arboles deben coincidir salvo la ruta purgada, que en el original existia y
    # en el reescrito ya no.
    $oldEntries = @(git ls-tree -r --name-only $old | Where-Object { $_ -ne $Path } | Sort-Object)
    $newEntries = @(git ls-tree -r --name-only $new | Sort-Object)
    if (($oldEntries -join "`n") -ne ($newEntries -join "`n")) {
        Write-Host "commit $i ($oldMsg): arboles distintos fuera de '$Path'" -ForegroundColor Red
        $failures++
        continue
    }

    # Y el contenido de cada fichero comun tiene que ser identico.
    $oldHash = (git rev-parse "$old^{tree}")
    $newHash = (git rev-parse "$new^{tree}")
    if ($oldHash -ne $newHash) {
        $diff = @(git diff --name-only $old $new | Where-Object { $_ -ne $Path })
        if ($diff.Count -gt 0) {
            Write-Host "commit $i ($oldMsg): difieren $($diff -join ', ')" -ForegroundColor Red
            $failures++
            continue
        }
    }
}

if ($failures -gt 0) {
    throw "$failures commits no equivalen al original"
}

Write-Host "OK: $($beforeCommits.Count) commits equivalentes; solo desaparecio '$Path'." -ForegroundColor Green
exit 0

