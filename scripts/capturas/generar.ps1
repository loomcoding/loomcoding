# Genera las capturas SVG de la documentación de usuario a partir de los volcados reales de la TUI
# (docs/mockups/*.md) y escribe docs/user/img/*.svg y docs/user/img/ORIGEN.md. La tabla de qué
# volcado, qué bloque y qué líneas usa cada imagen está en capturas.psd1, junto a este script.
#
# Es determinista: la misma entrada produce los mismos bytes (sin fechas ni rutas del equipo).
# Con -Check no escribe nada: regenera en memoria y falla si algún fichero del repo difiere
# (imagen, ORIGEN.md) o si sobra un SVG que la tabla no declara.
#
# Uso:
#   .\scripts\capturas\generar.ps1           genera las imágenes y ORIGEN.md
#   .\scripts\capturas\generar.ps1 -Check    comprueba que el repo coincide con lo que generaría

[CmdletBinding()]
param(
    [switch]$Check
)

$ErrorActionPreference = 'Stop'

# La raíz del repo es el padre del padre de este script (scripts/capturas/ -> scripts/ -> raíz).
$Raiz = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Img = Join-Path $Raiz 'docs\user\img'
$Utf8 = [System.Text.UTF8Encoding]::new($false)

# Métricas del SVG en píxeles. La celda de una fuente monoespaciada de 14 px mide unos 8,4 px.
$TamanoFuente = 14
$AnchoCelda = 8.4
$AltoLinea = 18
$Margen = 16
$Fondo = '#0d1117'
$ColorTexto = '#d0d7de'
$Familia = "'Cascadia Mono', 'Consolas', 'DejaVu Sans Mono', 'Liberation Mono', monospace"

# Anchura de visualización de un punto de código: 2 para los anchos (CJK, emoji), 1 para el resto.
function Test-AnchoDoble {
    param([int]$Codigo)
    return ($Codigo -ge 0x1100 -and $Codigo -le 0x115F) -or
        ($Codigo -ge 0x2E80 -and $Codigo -le 0x303E) -or
        ($Codigo -ge 0x3041 -and $Codigo -le 0x33FF) -or
        ($Codigo -ge 0x3400 -and $Codigo -le 0x4DBF) -or
        ($Codigo -ge 0x4E00 -and $Codigo -le 0x9FFF) -or
        ($Codigo -ge 0xAC00 -and $Codigo -le 0xD7A3) -or
        ($Codigo -ge 0xF900 -and $Codigo -le 0xFAFF) -or
        ($Codigo -ge 0xFE30 -and $Codigo -le 0xFE4F) -or
        ($Codigo -ge 0xFF00 -and $Codigo -le 0xFF60) -or
        ($Codigo -ge 0xFFE0 -and $Codigo -le 0xFFE6) -or
        ($Codigo -ge 0x1F300 -and $Codigo -le 0x1FAFF)
}

# Número de columnas de una línea, respetando los pares sustitutos.
function Get-AnchoCeldas {
    param([string]$Linea)
    $total = 0
    for ($i = 0; $i -lt $Linea.Length; $i++) {
        $c = $Linea[$i]
        $codigo = [int]$c
        if ([char]::IsHighSurrogate($c) -and ($i + 1) -lt $Linea.Length -and [char]::IsLowSurrogate($Linea[$i + 1])) {
            $codigo = [char]::ConvertToUtf32($c, $Linea[$i + 1])
            $i++
        }
        if (Test-AnchoDoble $codigo) { $total += 2 } else { $total += 1 }
    }
    return $total
}

# Filas de un bloque ```text de un volcado, recortadas al rango pedido. Si la línea de apertura
# lleva texto a continuación (```text loom ...), ese texto es la primera fila del bloque.
function Get-Filas {
    param([string]$Volcado, [int]$Numero, $Desde, $Hasta)
    $ruta = Join-Path $Raiz ($Volcado -replace '/', '\')
    if (-not (Test-Path -LiteralPath $ruta -PathType Leaf)) {
        throw "No existe el volcado de origen: $Volcado"
    }
    $lineas = [System.IO.File]::ReadAllText($ruta, $Utf8) -split '\r?\n'
    $bloques = 0
    $dentro = $false
    $filas = New-Object System.Collections.Generic.List[string]
    foreach ($linea in $lineas) {
        if (-not $dentro) {
            if ($linea -match '^```text(.*)$') {
                $bloques++
                if ($bloques -eq $Numero) {
                    $dentro = $true
                    if ($Matches[1] -ne '') { $filas.Add($Matches[1]) }
                }
            }
        }
        elseif ($linea -match '^```\s*$') {
            break
        }
        else {
            $filas.Add($linea)
        }
    }
    if (-not $dentro) {
        throw ('El volcado {0} no tiene un bloque ```text número {1}' -f $Volcado, $Numero)
    }
    $inicio = 1
    $fin = $filas.Count
    if ($null -ne $Desde) { $inicio = [int]$Desde }
    if ($null -ne $Hasta) { $fin = [int]$Hasta }
    if ($inicio -lt 1 -or $fin -gt $filas.Count -or $inicio -gt $fin) {
        throw ('Rango {0}-{1} fuera del bloque {2} de {3} ({4} líneas)' -f $inicio, $fin, $Numero, $Volcado, $filas.Count)
    }
    $recorte = New-Object System.Collections.Generic.List[string]
    for ($i = $inicio - 1; $i -lt $fin; $i++) {
        $recorte.Add(($filas[$i].Replace([string][char]9, '    ')).TrimEnd())
    }
    # Las filas vacías finales no aportan nada a la imagen.
    while ($recorte.Count -gt 0 -and $recorte[$recorte.Count - 1] -eq '') {
        $recorte.RemoveAt($recorte.Count - 1)
    }
    return , $recorte.ToArray()
}

# SVG de un bloque: fondo oscuro, una fila de texto por línea y tamaño según la fila más ancha.
function New-Svg {
    param([string]$Titulo, [string[]]$Filas)
    $columnas = 1
    foreach ($fila in $Filas) {
        $ancho = Get-AnchoCeldas $fila
        if ($ancho -gt $columnas) { $columnas = $ancho }
    }
    $anchoSvg = [int][math]::Ceiling($columnas * $AnchoCelda) + 2 * $Margen
    $altoSvg = $Filas.Count * $AltoLinea + 2 * $Margen
    $tituloEsc = [System.Security.SecurityElement]::Escape($Titulo)

    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append("<svg xmlns=`"http://www.w3.org/2000/svg`" width=`"$anchoSvg`" height=`"$altoSvg`" viewBox=`"0 0 $anchoSvg $altoSvg`" role=`"img`" aria-labelledby=`"titulo`">`n")
    [void]$sb.Append("<title id=`"titulo`">$tituloEsc</title>`n")
    [void]$sb.Append("<rect width=`"$anchoSvg`" height=`"$altoSvg`" rx=`"8`" fill=`"$Fondo`"/>`n")
    [void]$sb.Append("<g font-family=`"$Familia`" font-size=`"$TamanoFuente`" fill=`"$ColorTexto`" style=`"white-space:pre`">`n")
    for ($r = 0; $r -lt $Filas.Count; $r++) {
        $y = $Margen + ($r + 1) * $AltoLinea - 4
        $texto = [System.Security.SecurityElement]::Escape($Filas[$r])
        [void]$sb.Append("<text x=`"$Margen`" y=`"$y`" xml:space=`"preserve`">$texto</text>`n")
    }
    [void]$sb.Append("</g>`n</svg>`n")
    return $sb.ToString()
}

# Tabla de ORIGEN.md: imagen, volcado, bloque y líneas, prueba y comando para regenerar.
function New-OrigenMd {
    param($Imagenes)
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append("# Origen de las imágenes`n`n")
    [void]$sb.Append("Generadas por ``scripts/capturas/generar.ps1`` a partir de los volcados reales de ``docs/mockups/``.`n")
    [void]$sb.Append("No se editan a mano: para cambiarlas, cambie ``scripts/capturas/capturas.psd1`` o el volcado y regenere.`n`n")
    [void]$sb.Append("| Imagen | Volcado de origen | Bloque y líneas | Prueba que lo produce | Comando para regenerar |`n")
    [void]$sb.Append("|---|---|---|---|---|`n")
    foreach ($e in $Imagenes) {
        if ($null -ne $e.Desde) {
            $rango = "bloque $($e.Bloque), líneas $($e.Desde)-$($e.Hasta)"
        }
        else {
            $rango = "bloque $($e.Bloque), completo"
        }
        $prueba = "cargo test -p loomcoding-cli --test $($e.Prueba) -- --nocapture"
        [void]$sb.Append("| ``$($e.Archivo)`` | ``$($e.Volcado)`` | $rango | ``$prueba`` | ``.\scripts\capturas\generar.ps1`` |`n")
    }
    [void]$sb.Append("`nLos volcados de ``docs/mockups/`` se copian a mano desde ``target/tmp/e2e-shots/`` tras correr la prueba.`n")
    [void]$sb.Append("El generador solo lee ``docs/mockups/``; la prueba ``imagenes.rs`` comprueba que cada volcado existe.`n")
    return $sb.ToString()
}

# Compara sin distinguir finales de línea (CRLF o LF) y sin ignorar mayúsculas.
function Test-Igual {
    param([string]$A, [string]$B)
    return ($A -replace "`r`n", "`n") -ceq ($B -replace "`r`n", "`n")
}

$Tabla = Import-PowerShellDataFile -LiteralPath (Join-Path $PSScriptRoot 'capturas.psd1')
$Imagenes = @($Tabla.Imagenes)

$esperadas = [ordered]@{}
foreach ($e in $Imagenes) {
    $filas = Get-Filas -Volcado $e.Volcado -Numero $e.Bloque -Desde $e.Desde -Hasta $e.Hasta
    $esperadas[$e.Archivo] = New-Svg -Titulo $e.Titulo -Filas $filas
}
$origenEsperado = New-OrigenMd -Imagenes $Imagenes

if (-not $Check) {
    New-Item -ItemType Directory -Force -Path $Img | Out-Null
    foreach ($nombre in $esperadas.Keys) {
        [System.IO.File]::WriteAllText((Join-Path $Img $nombre), $esperadas[$nombre], $Utf8)
    }
    [System.IO.File]::WriteAllText((Join-Path $Img 'ORIGEN.md'), $origenEsperado, $Utf8)
    Write-Host "Generadas $($esperadas.Count) imágenes y ORIGEN.md en docs/user/img/."
    $sobran = @(Get-ChildItem -LiteralPath $Img -Filter *.svg | Where-Object { -not $esperadas.Contains($_.Name) })
    foreach ($s in $sobran) {
        Write-Host "Aviso: $($s.Name) no está en la tabla; bórrelo a mano si ya no hace falta." -ForegroundColor Yellow
    }
    exit 0
}

$difieren = New-Object System.Collections.Generic.List[string]
foreach ($nombre in $esperadas.Keys) {
    $ruta = Join-Path $Img $nombre
    if (-not (Test-Path -LiteralPath $ruta -PathType Leaf)) {
        $difieren.Add("falta docs/user/img/$nombre")
    }
    elseif (-not (Test-Igual ([System.IO.File]::ReadAllText($ruta, $Utf8)) $esperadas[$nombre])) {
        $difieren.Add("difiere docs/user/img/$nombre")
    }
}
$rutaOrigen = Join-Path $Img 'ORIGEN.md'
if (-not (Test-Path -LiteralPath $rutaOrigen -PathType Leaf)) {
    $difieren.Add('falta docs/user/img/ORIGEN.md')
}
elseif (-not (Test-Igual ([System.IO.File]::ReadAllText($rutaOrigen, $Utf8)) $origenEsperado)) {
    $difieren.Add('difiere docs/user/img/ORIGEN.md')
}
if (Test-Path -LiteralPath $Img -PathType Container) {
    foreach ($s in Get-ChildItem -LiteralPath $Img -Filter *.svg) {
        if (-not $esperadas.Contains($s.Name)) {
            $difieren.Add("sobra docs/user/img/$($s.Name) (no está en la tabla)")
        }
    }
}

if ($difieren.Count -gt 0) {
    foreach ($linea in $difieren) { Write-Host "FALLO: $linea" -ForegroundColor Red }
    Write-Host 'Regenere con .\scripts\capturas\generar.ps1 y revise el cambio.' -ForegroundColor Yellow
    exit 1
}
Write-Host "Las $($esperadas.Count) imágenes y ORIGEN.md coinciden con sus volcados." -ForegroundColor Green
exit 0
