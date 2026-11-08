# Regenera docs/assets/loomcoding-weave.gif desde la animación docs/assets/loomcoding-self-weaving.html.
#
# Cabe señalar que la animación tiene un modo de captura determinista (?capture&t=<ms>): semilla fija
# y tiempo explícito, de modo que dos ejecuciones producen los mismos fotogramas. El GIF final son
# 48 fotogramas de un ciclo de 1600 ms, a 200 ms cada uno, como el original.
#
# Requisitos: Google Chrome o Microsoft Edge, y ffmpeg en el PATH. Uso:
#   pwsh -File scripts/capturas/tejedora.ps1

$ErrorActionPreference = 'Stop'
$raiz = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$html = Join-Path $raiz 'docs\assets\loomcoding-self-weaving.html'
$destino = Join-Path $raiz 'docs\assets\loomcoding-weave.gif'
$fotogramas = 48
$cicloMs = 1600
$retardoMs = 200

$navegador = @(
    'C:\Program Files\Google\Chrome\Application\chrome.exe',
    'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $navegador) { throw 'No se encuentra Chrome ni Edge.' }
if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue)) { throw 'No se encuentra ffmpeg en el PATH.' }

$tmp = Join-Path ([IO.Path]::GetTempPath()) ('tejedora-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $base = ([Uri]$html).AbsoluteUri
    for ($i = 0; $i -lt $fotogramas; $i++) {
        $t = [int][Math]::Round($i * $cicloMs / $fotogramas)
        $png = Join-Path $tmp ('f{0:D2}.png' -f $i)
        & $navegador --headless=new --disable-gpu --hide-scrollbars --user-data-dir="$tmp\perfil" `
            --window-size=776,606 --screenshot="$png" ($base + '?capture&t=' + $t) 2>&1 | Out-Null
        if (-not (Test-Path $png)) { throw "No se generó el fotograma $i." }
    }
    $fps = 1000 / $retardoMs
    ffmpeg -v error -y -framerate $fps -i "$tmp\f%02d.png" `
        -vf 'scale=520:406:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=3' `
        -loop 0 $destino
    if ($LASTEXITCODE -ne 0) { throw 'ffmpeg falló al componer el GIF.' }
    "GIF regenerado: $destino ($([Math]::Round((Get-Item $destino).Length / 1KB)) KB)"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
