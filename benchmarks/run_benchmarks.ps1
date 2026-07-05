# Benchmark Runner — Forja vs Raven vs Rust Native (AOT)
# Ejecuta cada binario N veces y reporta estadísticas

$Iterations = 10
$WarmupIterations = 3

Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  BENCHMARK AOT: Forja (transpile→rustc -O) vs Raven vs Rust" -ForegroundColor Cyan
Write-Host "  Iteraciones: $Iterations (warmup: $WarmupIterations)" -ForegroundColor Cyan
Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan

$BaseDir = "C:\Users\gaucho\forja"
$Binaries = @(
    @{ Name = "Forja AOT (transpile→rustc -O)"; Path = "$BaseDir\benchmarks\bench_forja_aot.rs\target\release\bench_forja_aot.exe" },
    @{ Name = "Raven AOT (Cranelift)"; Path = "$BaseDir\benchmarks\bench_raven_rv.exe" },
    @{ Name = "Rust Native (rustc -O)"; Path = "$BaseDir\benchmarks\bench_rust_native_aot.exe" }
)

function Measure-Benchmark {
    param($Binary, $Iterations, $Warmup)

    $times = @()

    # Warmup
    Write-Host "  🏋️  Warmup..." -NoNewline
    for ($i = 0; $i -lt $Warmup; $i++) {
        $null = Start-Process -FilePath $Binary -NoNewWindow -Wait -RedirectStandardOutput "NUL"
    }
    Write-Host " done" -ForegroundColor Green

    # Actual measurements
    for ($i = 0; $i -lt $Iterations; $i++) {
        $ms = Measure-Command { 
            $null = Start-Process -FilePath $Binary -NoNewWindow -Wait -RedirectStandardOutput "NUL"
        }
        $times += $ms.TotalMilliseconds
        Write-Host "  ⏱️  Iteración $($i+1): $([math]::Round($ms.TotalMilliseconds, 4)) ms" -ForegroundColor Gray
    }

    $avg = ($times | Measure-Object -Average).Average
    $min = ($times | Measure-Object -Minimum).Minimum
    $max = ($times | Measure-Object -Maximum).Maximum
    $stddev = [math]::Sqrt(($times | ForEach-Object { ($_ - $avg) * ($_ - $avg) } | Measure-Object -Average).Average)

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
    Write-Host "    Promedio: $([math]::Round($result.Average, 4)) ms" -ForegroundColor White
    Write-Host "    Mínimo:   $([math]::Round($result.Min, 4)) ms" -ForegroundColor White
    Write-Host "    Máximo:   $([math]::Round($result.Max, 4)) ms" -ForegroundColor White
    Write-Host "    StdDev:   $([math]::Round($result.StdDev, 4)) ms" -ForegroundColor White
}

# Summary Table
Write-Host ""
Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  📋 TABLA COMPARATIVA" -ForegroundColor Cyan
Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

$rustTime = $Results["Rust Native (rustc -O)"].Average

Write-Host ("{0,-40} {1,12} {2,14}" -f "Implementación", "Promedio (ms)", "vs Rust (x)")
Write-Host ("{0,-40} {1,12} {2,14}" -f ("─" * 40), ("─" * 12), ("─" * 14))

$sortedResults = $Results.GetEnumerator() | Sort-Object { $_.Value.Average }

foreach ($entry in $sortedResults) {
    $name = $entry.Key
    $avg = $entry.Value.Average
    $ratio = if ($rustTime -gt 0) { $avg / $rustTime } else { 1.0 }
    $ratioStr = if ($name -eq "Rust Native (rustc -O)") { "1.00x (ref)" } else { "$([math]::Round($ratio, 2))x" }
    Write-Host ("{0,-40} {1,10:F4} ms {2,14}" -f $name, $avg, $ratioStr)
}

Write-Host ""
Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""
Write-Host "🔍 Nota: Menor tiempo = más rápido. 'vs Rust' indica cuántas" -ForegroundColor Gray
Write-Host "   veces más lento es respecto a Rust nativo compilado con rustc -O." -ForegroundColor Gray

# ============================================================
# NUEVOS BENCHMARKS: Ejecutar con el intérprete/compilador Forja
# ============================================================
Write-Host ""
Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  BENCHMARKS FORJA (intérprete / runtime)" -ForegroundColor Cyan
Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan

$ForjaRunner = "$BaseDir\target\release\forja.exe"

# Función para ejecutar un benchmark .fa y medir tiempo
function Measure-ForjaBenchmark {
    param($ScriptPath, $Label)

    if (-not (Test-Path $ForjaRunner)) {
        Write-Host "  ❌ Forja runner no encontrado: $ForjaRunner" -ForegroundColor Red
        return
    }
    if (-not (Test-Path $ScriptPath)) {
        Write-Host "  ❌ Script no encontrado: $ScriptPath" -ForegroundColor Red
        return
    }

    Write-Host ""
    Write-Host "╔══ $Label ═══╗" -ForegroundColor Yellow
    Write-Host "║ $ScriptPath" -ForegroundColor Yellow
    Write-Host "╚══════════════════════════════════════╝" -ForegroundColor Yellow

    # Warmup
    Write-Host "  🏋️  Warmup..." -NoNewline
    for ($i = 0; $i -lt $WarmupIterations; $i++) {
        $null = Start-Process -FilePath $ForjaRunner -ArgumentList "run", $ScriptPath -NoNewWindow -Wait -RedirectStandardOutput "NUL" -RedirectStandardError "NUL"
    }
    Write-Host " done" -ForegroundColor Green

    $times = @()
    for ($i = 0; $i -lt $Iterations; $i++) {
        $ms = Measure-Command {
            $null = Start-Process -FilePath $ForjaRunner -ArgumentList "run", $ScriptPath -NoNewWindow -Wait -RedirectStandardOutput "NUL" -RedirectStandardError "NUL"
        }
        $times += $ms.TotalMilliseconds
        Write-Host "  ⏱️  Iteración $($i+1): $([math]::Round($ms.TotalMilliseconds, 4)) ms" -ForegroundColor Gray
    }

    $avg = ($times | Measure-Object -Average).Average
    $min = ($times | Measure-Object -Minimum).Minimum
    $max = ($times | Measure-Object -Maximum).Maximum
    $stddev = [math]::Sqrt(($times | ForEach-Object { ($_ - $avg) * ($_ - $avg) } | Measure-Object -Average).Average)

    Write-Host ""
    Write-Host "  📊 Resultados:" -ForegroundColor Green
    Write-Host "    Promedio: $([math]::Round($avg, 4)) ms" -ForegroundColor White
    Write-Host "    Mínimo:   $([math]::Round($min, 4)) ms" -ForegroundColor White
    Write-Host "    Máximo:   $([math]::Round($max, 4)) ms" -ForegroundColor White
    Write-Host "    StdDev:   $([math]::Round($stddev, 4)) ms" -ForegroundColor White
}

# === Ejecutar benchmarks existentes ===
Write-Host ""
Write-Host "────────────────────────────────────────────────────────────" -ForegroundColor Cyan
Write-Host "  BENCHMARKS EXISTENTES" -ForegroundColor Cyan
Write-Host "────────────────────────────────────────────────────────────" -ForegroundColor Cyan
Measure-ForjaBenchmark -ScriptPath "$BaseDir\benchmarks\bench_forja_heavy.fa" -Label "Benchmark Heavy Existente"

# === Nuevos benchmarks ===
Write-Host ""
Write-Host "────────────────────────────────────────────────────────────" -ForegroundColor Cyan
Write-Host "  NUEVOS BENCHMARKS (Fase 6.3)" -ForegroundColor Cyan
Write-Host "────────────────────────────────────────────────────────────" -ForegroundColor Cyan

Write-Host ""
Write-Host "=== Benchmark LLVM Backend ===" -ForegroundColor Yellow
Measure-ForjaBenchmark -ScriptPath "$BaseDir\benchmarks\bench_llvm.fa" -Label "LLVM Backend"

Write-Host ""
Write-Host "=== Benchmark Concurrencia ===" -ForegroundColor Yellow
Measure-ForjaBenchmark -ScriptPath "$BaseDir\benchmarks\bench_concurrencia.fa" -Label "Concurrencia"

Write-Host ""
Write-Host "=== Benchmark FFI ===" -ForegroundColor Yellow
Measure-ForjaBenchmark -ScriptPath "$BaseDir\benchmarks\bench_ffi.fa" -Label "FFI"

Write-Host ""
Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  BENCHMARKS COMPLETADOS" -ForegroundColor Cyan
Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
