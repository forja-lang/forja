# Script para compilar Forja a WebAssembly
# Requiere: wasm-pack (cargo install wasm-pack)
#
# Uso: .\scripts\build-wasm.ps1

$ErrorActionPreference = "Stop"

Write-Host "🔨 Compilando Forja a WebAssembly..." -ForegroundColor Cyan

# Ir al directorio del crate WASM
Push-Location (Join-Path $PSScriptRoot "..\crates\forja-wasm")

try {
    # Compilar con wasm-pack para target web
    wasm-pack build --target web --release

    Write-Host "✅ WASM compilado exitosamente!" -ForegroundColor Green

    # Copiar los archivos generados al directorio public de la documentación
    $pkgDir = Join-Path (Get-Location) "pkg"
    $publicWasmDir = Join-Path $PSScriptRoot "..\docs\public\wasm"

    if (-not (Test-Path $publicWasmDir)) {
        New-Item -ItemType Directory -Path $publicWasmDir -Force | Out-Null
    }

    Copy-Item (Join-Path $pkgDir "forja_wasm.js") (Join-Path $publicWasmDir "forja_wasm.js") -Force
    Copy-Item (Join-Path $pkgDir "forja_wasm_bg.wasm") (Join-Path $publicWasmDir "forja_wasm_bg.wasm") -Force
    Copy-Item (Join-Path $pkgDir "forja_wasm.d.ts") (Join-Path $publicWasmDir "forja_wasm.d.ts") -Force

    Write-Host "📦 Archivos copiados a docs/public/wasm/" -ForegroundColor Green
    Write-Host ""
    Write-Host "📊 Tamaños:" -ForegroundColor Yellow
    Get-ChildItem $publicWasmDir | ForEach-Object {
        $sizeInKB = [math]::Round($_.Length / 1KB, 2)
        Write-Host "   $($_.Name): $sizeInKB KB"
    }
}
catch {
    Write-Host "❌ Error compilando WASM: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Asegurate de tener wasm-pack instalado:" -ForegroundColor Yellow
    Write-Host "  cargo install wasm-pack" -ForegroundColor White
    Write-Host ""
    Write-Host "O descargalo desde: https://rustwasm.github.io/wasm-pack/installer/" -ForegroundColor Yellow
}
finally {
    Pop-Location
}
