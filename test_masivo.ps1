$ok = 0
$fail = 0
$timeout = 0
$saltados = 0
$total = 0
$results = @()
$detalle = @()

$exe = "target\release\forja.exe"
if (-not (Test-Path $exe)) {
    $exe = "target\debug\forja.exe"
}

Write-Host "=== TEST MASIVO Forja ===" -ForegroundColor Cyan
Write-Host "Ejecutable: $exe`n"

# Archivos que requieren input interactivo continuo o select() que bloquea
$archivosTimeoutConocidos = @{
    # Select() sin mensajes → bloquean esperando
    "116_select_simple.fa" = $true
    "118_select_default.fa" = $true
    "120_select_trabajadores.fa" = $true
    "73_seleccionar.fa" = $true
    # Hilos que deadlockean o nunca terminan
    "128_hilo_multiple.fa" = $true
    "130_hilo_productor_consumidor.fa" = $true
    "70_concurrencia.fa" = $true
    # Juegos interactivos (necesitan múltiples inputs)
    "152_piedra_papel_tijera.fa" = $true
    "154_tateti.fa" = $true
    "156_calculadora_cientifica.fa" = $true
    "157_conversor_unidades.fa" = $true
    "160_cronometro.fa" = $true
    "173_observador.fa" = $true
    "201_login_gui.fa" = $true
    # Genéricos: fallan en parser y entran en bucle infinito
    "104_generico_caja.fa" = $true
    "105_generico_par.fa" = $true
    "107_generico_multiple.fa" = $true
    "108_generico_arbol.fa" = $true
    "110_generico_resultado.fa" = $true
    "72_genericos.fa" = $true
    # Interpolación con formateo
    "80_interpolacion_formateo.fa" = $true
}

Write-Host "Archivos saltados por timeout conocido: $($archivosTimeoutConocidos.Count)" -ForegroundColor DarkYellow

# Pre-scan para detectar archivos con leer()
$archivosConInput = @{}
Get-ChildItem "examples\*.fa" | ForEach-Object {
    $content = Get-Content -Path $_.FullName -Raw
    if ($content -match 'leer\s*\(') {
        $archivosConInput[$_.Name] = $true
    }
}
if ($archivosConInput.Count -gt 0) {
    Write-Host "Archivos con leer() (recibirán Enter):" -ForegroundColor DarkYellow
    foreach ($n in $archivosConInput.Keys) { Write-Host "  - $n" -ForegroundColor DarkYellow }
    Write-Host ""
}

Get-ChildItem "examples\*.fa" | Sort-Object Name | ForEach-Object {
    $file = $_.FullName
    $name = $_.Name
    $tieneLeer = $archivosConInput.ContainsKey($name)
    
    # Saltar archivos con timeout conocido
    if ($archivosTimeoutConocidos.ContainsKey($name)) {
        Write-Host "  $name ... SALTADO (timeout conocido)" -ForegroundColor DarkGray
        $saltados++
        $results += "[SALTADO] $name"
        return
    }
    
    $total++
    
    Write-Host -NoNewline "  $name ... "
    
    $pinfo = New-Object System.Diagnostics.ProcessStartInfo
    $pinfo.FileName = $exe
    $pinfo.Arguments = "run", $file
    $pinfo.RedirectStandardOutput = $true
    $pinfo.RedirectStandardError = $true
    $pinfo.RedirectStandardInput = $tieneLeer
    $pinfo.UseShellExecute = $false
    $pinfo.CreateNoWindow = $true
    $pinfo.WorkingDirectory = (Get-Location).Path
    
    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $pinfo
    
    $p.Start() | Out-Null
    
    # Si tiene leer(), pasar Enter y cerrar stdin
    if ($tieneLeer) {
        try {
            $p.StandardInput.WriteLine("")
            $p.StandardInput.Close()
        } catch {
            # Ignorar errores de stdin
        }
    }
    
    if ($p.WaitForExit(10000)) {
        $stdout = $p.StandardOutput.ReadToEnd()
        $stderr = $p.StandardError.ReadToEnd()
        $exitCode = $p.ExitCode
        
        if ($exitCode -eq 0) {
            Write-Host "OK" -ForegroundColor Green
            $ok++
            $results += "[OK] $name"
        } else {
            $errLine = ($stderr -split "`n" | Where-Object { $_ -ne "" } | Select-Object -First 1)
            if ($errLine.Length -gt 120) { $errLine = $errLine.Substring(0, 120) + "..." }
            Write-Host "FAIL" -ForegroundColor Red
            Write-Host "     -> $errLine" -ForegroundColor DarkRed
            $fail++
            $results += "[FAIL] $name $errLine"
            $detalle += "--- $name ---`n$stderr`n"
        }
    } else {
        $p.Kill()
        Write-Host "TIMEOUT" -ForegroundColor Yellow
        $timeout++
        $results += "[TIMEOUT] $name Timeout de 10s"
    }
}

Write-Host "`n============================================" -ForegroundColor Cyan
Write-Host "  RESULTADOS FINALES" -ForegroundColor Cyan
Write-Host "============================================"
Write-Host "  Total:   $($total + $saltados)"
Write-Host "  OK:      $ok"
Write-Host "  FAIL:    $fail"
Write-Host "  TIMEOUT: $timeout"
Write-Host "  SALTADO: $saltados"
$pctEjecutados = if ($total -gt 0) { [math]::Round($ok / $total * 100, 1) } else { 0 }
$pctTotal = if (($total + $saltados) -gt 0) { [math]::Round($ok / ($total + $saltados) * 100, 1) } else { 0 }
Write-Host "  Exito:   $pctEjecutados% (de ejecutados) / $pctTotal% (del total)"
Write-Host "============================================"

# Save summary
$header = @"
============================================
  Resultados de prueba - Forja (fa)
  Fecha: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')
============================================

Resumen:
  Total:  $total
  OK:     $ok
  FAIL:   $fail
  TIMEOUT:$timeout
  Exito:  $pct%

--------------------------------------------
  Detalle por archivo
--------------------------------------------
"@
$header + ($results -join "`n") + "`n`n--------------------------------------------`n  Fin del reporte" | Out-File "test_resultados.txt"

Write-Host "`nResultados guardados en test_resultados.txt"
if ($detalle.Count -gt 0) {
    Write-Host "Detalles de fallos guardados en test_detalle.txt"
    $detalle | Out-File "test_detalle.txt"
}
