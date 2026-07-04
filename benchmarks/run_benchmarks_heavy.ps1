# Benchmark Runner Heavy — Forja vs Raven vs Rust Native (AOT)
# Cada binario ejecuta 100 iteraciones internas de cada test
# Medimos tiempo total con Measure-Command

$Iterations = 7
$WarmupIterations = 2

Write-Host "══════════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  BENCHMARK AOT HEAVY: Forja (transpile→rustc -O) vs Raven vs Rust" -ForegroundColor Cyan
Write-Host "  Cada binario ejecuta 100 iteraciones internas de cada test" -ForegroundColor Cyan
Write-Host "  Iteraciones externas: $Iterations (warmup: $WarmupIterations)" -ForegroundColor Cyan
Write-Host "══════════════════════════════════════════════════════════════════════" -ForegroundColor Cyan

$BaseDir = "C:\Users\gaucho\forja"
$Binaries = @(
    @{ Name = "Forja AOT (transpile→rustc -O)"; Path = "$BaseDir\benchmarks\bench_forja_heavy.rs\target\release\bench_forja_heavy.exe" },
    @{ Name = "Raven AOT (Cranelift)"; Path = "$BaseDir\benchmarks\bench_raven_heavy.exe" },
    @{ Name = "Rust Native (rustc -O)"; Path = "$BaseDir\benchmarks\bench_rust_heavy.exe" }
)

function Measure-Benchmark {
    param($Binary, $Iterations, $Warmup)

    $times = @()

    # Warmup
    Write-Host "  🏋️  Warmup..." -NoNewline
    for ($i = 0; $i -lt $Warmup; $i++) {
        $null = Start-Process -FilePath $Binary -NoNewWindow -Wait -RedirectStandardOutput "NUL" -RedirectStandardError "NUL"
    }
    Write-Host " done" -ForegroundColor Green

    # Actual measurements
    for ($i = 0; $i -lt $Iterations; $i++) {
        $ms = Measure-Command { 
            $null = Start-Process -FilePath $Binary -NoNewWindow -Wait -RedirectStandardOutput "NUL" -RedirectStandardError "NUL"
        }
        $times += $ms.TotalMilliseconds
        Write-Host "  ⏱️  Iteración $($i+1): $([math]::Round($ms.TotalMilliseconds, 2)) ms" -ForegroundColor Gray
        Start-Sleep -Milliseconds 200
    }

    $avg = [math]::Round(($times | Measure-Object -Average).Average, 2)
    $min = [math]::Round(($times | Measure-Object -Minimum).Minimum, 2)
    $max = [math]::Round(($times | Measure-Object -Maximum).Maximum, 2)
    $variance = ($times | ForEach-Object { ($_ - $avg) * ($_ - $avg) } | Measure-Object -Average).Average
    $stddev = [math]::Round([math]::Sqrt($variance), 2)

    return @{
        Average = $avg
        Min = $min
        Max = $max
        StdDev = $stddev
        AllTimes = $times
    }
}

$Results = @{}

foreach ($bin in $Binaries) {
    Write-Host ""
    Write-Host "╔══ $($bin.Name) ═══╗" -ForegroundColor Yellow
    Write-Host "║ $($bin.Path)" -ForegroundColor Yellow
    Write-Host "╚══════════════════════════════════════╝" -ForegroundColor Yellow

    if (-not (Test-Path $bin.Path)) {
        Write-Host "  ❌ Binario no encontrado: $($bin.Path)" -ForegroundColor Red
        continue
    }

    $result = Measure-Benchmark -Binary $bin.Path -Iterations $Iterations -Warmup $WarmupIterations
    $Results[$bin.Name] = $result

    Write-Host ""
    Write-Host "  📊 Resultados:" -ForegroundColor Green
    Write-Host "    Promedio: $($result.Average) ms" -ForegroundColor White
    Write-Host "    Mínimo:   $($result.Min) ms" -ForegroundColor White
    Write-Host "    Máximo:   $($result.Max) ms" -ForegroundColor White
    Write-Host "    StdDev:   $($result.StdDev) ms" -ForegroundColor White
}

Write-Host ""
Write-Host "══════════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  📋 TABLA COMPARATIVA" -ForegroundColor Cyan
Write-Host "══════════════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""
Write-Host ("┌──────────────────────────────────────────────┬──────────────┬──────────────┐") -ForegroundColor Gray
Write-Host ("│ {0,-44} │ {1,12} │ {2,12} │" -f "Implementación", "Promedio (ms)", "vs Rust (x)") -ForegroundColor Gray
Write-Host ("├──────────────────────────────────────────────┼──────────────┼──────────────┤") -ForegroundColor Gray

$rustTime = $Results["Rust Native (rustc -O)"].Average

$sortedResults = $Results.GetEnumerator() | Sort-Object { $_.Value.Average }

foreach ($entry in $sortedResults) {
    $name = $entry.Key
    $avg = $entry.Value.Average
    $ratio = if ($rustTime -gt 0) { $avg / $rustTime } else { 1.0 }
    $ratioStr = if ($name -eq "Rust Native (rustc -O)") { "1.00x (ref)" } else { "$([math]::Round($ratio, 2))x" }
    Write-Host ("│ {0,-44} │ {1,10:F2} ms │ {2,12} │" -f $name, $avg, $ratioStr)
}

Write-Host ("└──────────────────────────────────────────────┴──────────────┴──────────────┘") -ForegroundColor Gray
Write-Host ""
Write-Host "🔍 Menor tiempo = más rápido. 'vs Rust' indica cuántas veces más lento." -ForegroundColor Gray
Write-Host "   Todos compilados en modo Release/AOT optimizado." -ForegroundColor Gray
Write-Host "   Forja AOT: transpilación a Rust + rustc -O (vía 'forja transpile')" -ForegroundColor Gray
Write-Host "   Raven AOT: compilación nativa vía Cranelift" -ForegroundColor Gray
Write-Host "   Rust: compilación directa con rustc -O" -ForegroundColor Gray
