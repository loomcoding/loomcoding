<#
.SYNOPSIS
Sirve por web un comando de terminal con ttyd (xterm.js), solo en 127.0.0.1, para
abrirlo con Chrome y capturar su interfaz.

.EXAMPLE
pwsh -NoProfile -File scripts/visual/serve-tui.ps1 -Port 7681 -Cwd E:\proyecto -Run 'loomcoding.exe --yolo'

.EXAMPLE
pwsh -NoProfile -File scripts/visual/serve-tui.ps1 -Port 7681 -Stop

Las variables de entorno que deba ver el comando (por ejemplo LOOMCODING_HOME) se definen
en el shell que invoca este script: ttyd y su hijo las heredan.
Sale con 1 si ttyd no se encuentra o el puerto está ocupado.
#>
param(
    [int]$Port = 7681,
    [string]$Cwd = (Get-Location).Path,
    [string]$TtydPath,
    [int]$FontSize = 14,
    [switch]$Stop,
    # Línea de órdenes completa del programa a servir, tal como se escribiría en un shell.
    [string]$Run
)

$ErrorActionPreference = 'Stop'
$pidFile = Join-Path $env:TEMP "serve-tui-$Port.pid"

function Fail([string]$message) {
    [Console]::Error.WriteLine("serve-tui: $message")
    exit 1
}

if ($Stop) {
    if (-not (Test-Path $pidFile)) { Fail "no hay un servidor registrado en el puerto $Port." }
    $serverPid = [int](Get-Content $pidFile)
    Stop-Process -Id $serverPid -Force -ErrorAction SilentlyContinue
    Remove-Item $pidFile -Force
    Write-Host "detenido ttyd (PID $serverPid) del puerto $Port"
    exit 0
}

# winget añade ttyd al PATH de usuario, que un shell ya abierto no ve hasta reiniciarse.
$env:Path = "$([Environment]::GetEnvironmentVariable('Path','Machine'));$([Environment]::GetEnvironmentVariable('Path','User'))"
if (-not $TtydPath) {
    $found = Get-Command ttyd -ErrorAction SilentlyContinue
    if ($found) { $TtydPath = $found.Source }
}
if (-not $TtydPath -or -not (Test-Path $TtydPath)) {
    Fail "no se encuentra ttyd. Instálalo con: winget install tsl0922.ttyd"
}

if ([string]::IsNullOrWhiteSpace($Run)) { Fail "falta el comando a servir (-Run '<línea de órdenes>')." }

if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    Fail "el puerto $Port está ocupado."
}

$ttydArgs = "-W -i 127.0.0.1 -p $Port -t fontSize=$FontSize $Run"
$process = Start-Process -FilePath $TtydPath -ArgumentList $ttydArgs `
    -WorkingDirectory $Cwd -WindowStyle Hidden -PassThru
Set-Content -Path $pidFile -Value $process.Id

$deadline = (Get-Date).AddSeconds(8)
while ((Get-Date) -lt $deadline) {
    if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
        Write-Host "http://127.0.0.1:$Port  (PID $($process.Id); detener con -Stop -Port $Port)"
        exit 0
    }
    if ($process.HasExited) { break }
    Start-Sleep -Milliseconds 100
}
Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
Fail "ttyd no llegó a escuchar en el puerto $Port."
