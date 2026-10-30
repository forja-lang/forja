<#
.SYNOPSIS
    Instalador de Forja para Windows (F3-T7).

.DESCRIPTION
    Descarga el paquete precompilado (.zip) de la última release (o de una
    versión concreta), verifica su SHA-256 y deja los binarios en el directorio
    de instalación. NO requiere Rust ni compilar.

.EXAMPLE
    irm https://raw.githubusercontent.com/forja-lang/forja/main/scripts/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Version v0.9.5 -InstallDir "$env:LOCALAPPDATA\forja"
#>
param(
    [string]$Version = "latest",
    [string]$InstallDir = "$env:LOCALAPPDATA\forja",
    [switch]$NoPath
)

$ErrorActionPreference = "Stop"
$Repo = if ($env:FORJA_REPO) { $env:FORJA_REPO } else { "forja-lang/forja" }

function Write-Step($msg) { Write-Host "🔥 $msg" -ForegroundColor Yellow }
function Write-Ok($msg)   { Write-Host "   ✅ $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "   ⚠️  $msg" -ForegroundColor Yellow }

# ── Detectar arquitectura ──
$archRaw = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
switch ($archRaw) {
    "X64"   { $arch = "x64" }
    "Arm64" { $arch = "arm64" }
    default { throw "❌ Arquitectura no soportada: $archRaw" }
}
$name = "windows-$arch"

# ── Resolver versión/tag ──
if ($Version -eq "latest") {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "forja-installer" }
    $tag = $release.tag_name
} else {
    $tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
}
if (-not $tag) { throw "❌ No se pudo determinar la versión." }
$ver = $tag.TrimStart("v")
$asset = "forja-$ver-$name.zip"
$base = "https://github.com/$Repo/releases/download/$tag"

Write-Step "Instalando Forja $tag ($name) en $InstallDir"

$tmp = Join-Path $env:TEMP ("forja-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
    $zipPath = Join-Path $tmp $asset
    Write-Host "   ⬇️  Descargando $asset ..."
    Invoke-WebRequest -Uri "$base/$asset" -OutFile $zipPath -UseBasicParsing

    # ── Verificar SHA-256 ──
    $sumsPath = Join-Path $tmp "SHA256SUMS"
    $haveSums = $true
    try {
        Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile $sumsPath -UseBasicParsing
    } catch {
        $haveSums = $false
        Write-Warn "SHA256SUMS no disponible; se omite la verificación."
    }
    if ($haveSums) {
        $line = Select-String -Path $sumsPath -Pattern ([regex]::Escape(" $asset")) | Select-Object -First 1
        if ($line) {
            $expected = ($line.Line -split '\s+')[0].ToLower()
            $actual = (Get-FileHash -Algorithm SHA256 -Path $zipPath).Hash.ToLower()
            if ($expected -ne $actual) {
                throw "❌ Checksum inválido para $asset`n   esperado: $expected`n   obtenido: $actual"
            }
            Write-Ok "Checksum verificado"
        }
    }

    # ── F6-T7: verificar firma cosign (si está instalado) ──
    if (Get-Command cosign -ErrorAction SilentlyContinue) {
        $bundle = Join-Path $tmp "$asset.bundle"
        try {
            Invoke-WebRequest -Uri "$base/$asset.bundle" -OutFile $bundle -UseBasicParsing
            & cosign verify-blob `
                --bundle $bundle `
                --certificate-identity-regexp "https://github.com/$Repo/.github/workflows/rust.yml@refs/heads/(main|master)" `
                --certificate-oidc-issuer "https://token.actions.githubusercontent.com" `
                $zipPath | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "Firma cosign inválida para $asset" }
            Write-Ok "Firma cosign verificada"
        } catch {
            throw "❌ Firma cosign inválida para ${asset}: $($_.Exception.Message)"
        }
    } else {
        Write-Warn "cosign no instalado; se omite la verificación de firma (ver SECURITY.md)."
    }

    # ── Extraer e instalar ──
    $extract = Join-Path $tmp "extract"
    Expand-Archive -Path $zipPath -DestinationPath $extract -Force
    $pkg = Join-Path $extract "forja-$ver-$name"
    if (-not (Test-Path $pkg)) { throw "❌ El paquete no tiene la estructura esperada." }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Path (Join-Path $pkg "*") -Destination $InstallDir -Recurse -Force
    Write-Ok "Archivos copiados a $InstallDir"

    # ── PATH de usuario ──
    if (-not $NoPath) {
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if (-not $userPath) { $userPath = "" }
        if ($userPath -notlike "*$InstallDir*") {
            $newPath = if ([string]::IsNullOrEmpty($userPath)) { $InstallDir } else { "$userPath;$InstallDir" }
            [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
            Write-Ok "Agregado al PATH de usuario (reiniciá la terminal para que tome efecto)"
        } else {
            Write-Ok "$InstallDir ya está en el PATH"
        }
        $env:Path = "$env:Path;$InstallDir"
    }

    Write-Step "Forja instalado en $InstallDir"
    $exe = Join-Path $InstallDir "forja.exe"
    if (Test-Path $exe) { & $exe --version }
    Write-Host "🎉 ¡Listo! Probá: forja run ejemplos\01_hola.fa" -ForegroundColor Green
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
