# ============================================================
# scripts/bump-version.ps1 — Bump version tool for Windows / PowerShell
#
# Updates version number across all Forja workspace crates,
# docs, benchmarks, VSCode extension, build scripts, and installer.
#
# Usage:
#   .\scripts\bump-version.ps1 0.9.3
#   .\scripts\bump-version.ps1 0.9.3 -GitMode commit
#   .\scripts\bump-version.ps1 0.9.3 -GitMode tag
#   .\scripts\bump-version.ps1 0.9.3 -GitMode push
#   .\scripts\bump-version.ps1 0.9.3 -GitMode all
# ============================================================
param(
    [Parameter(Position=0, Mandatory=$true)]
    [string]$NewVersion,

    [Parameter(Position=1, Mandatory=$false)]
    [string]$GitMode = "none",

    # Versión vieja para reemplazar en los textos (README, docs, scripts).
    # Hace falta cuando la raíz ya se movió pero quedaron textos desfasados.
    [Parameter(Mandatory=$false)]
    [string]$From = ""
)

$ErrorActionPreference = "Stop"

# Standardize GitMode options
$GitMode = $GitMode.TrimStart("-").ToLower()
if ($GitMode -notin @("none", "commit", "tag", "push", "all")) {
    Write-Error "❌ Error: Modo git inválido '$GitMode'. Usar: none, commit, tag, push o all"
    exit 1
}

# Validate SemVer format
if ($NewVersion -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$') {
    Write-Error "❌ Error: '$NewVersion' no es una versión semver válida (esperado: X.Y.Z)"
    exit 1
}

$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path | Split-Path -Parent
Set-Location $ProjectDir

$CargoToml = Join-Path $ProjectDir "Cargo.toml"
if (-not (Test-Path $CargoToml)) {
    Write-Error "❌ Error: Debe ejecutarse desde la raíz del repositorio de Forja"
    exit 1
}

# Detect current version.
#
# La raíz puede ser un workspace virtual (sin `[package]`): ahí la versión vive
# en `[workspace.package]` o, si tampoco, en los crates miembros.
function Get-VersionDe {
    param([string]$Texto)
    $m = [regex]::Match($Texto, '(?m)^version\s*=\s*"([^"]+)"')
    if ($m.Success) { return $m.Groups[1].Value }
    return ""
}

$Content = Get-Content $CargoToml -Raw

# Lista de crates del workspace: `members` + `default-members`.
# Cargo acepta en `default-members` crates que no están en `members`
# (`compiler/forja`, `src/tools/forja-repl`) y quedaban en la versión vieja.
function Get-Miembros {
    param([string]$Clave)
    [regex]::Matches([regex]::Match($Content, "(?ms)^$Clave\s*=\s*\[(.*?)\]").Groups[1].Value,
        '"([^"]+)"') | ForEach-Object { $_.Groups[1].Value }
}
$WorkspaceCrates = @(Get-Miembros 'members') + @(Get-Miembros 'default-members') |
    Select-Object -Unique

$OldVersion = Get-VersionDe $Content
if (-not $OldVersion) {
    # Un bump anterior pudo dejar crates desfasados: manda la versión
    # mayoritaria del workspace, no la del primer crate de la lista.
    $OldVersion = ($WorkspaceCrates | ForEach-Object {
            $toml = Join-Path $ProjectDir "$_\Cargo.toml"
            if (-not (Test-Path $toml)) { return }
            Get-VersionDe (Get-Content $toml -Raw)
        } | Where-Object { $_ } | Group-Object | Sort-Object Count -Descending |
        Select-Object -First 1).Name
}
if (-not $OldVersion) {
    Write-Error "❌ Error: No se pudo detectar la versión en Cargo.toml ni en los crates del workspace"
    exit 1
}

# -From manda: sirve para reparar textos cuando la raíz ya está en la nueva.
if ($From) { $OldVersion = $From }

if ($OldVersion -eq $NewVersion) {
    Write-Host "⚠️ La versión ya es $NewVersion." -ForegroundColor Yellow
    Write-Host "   Se revisan igual todos los archivos para reparar lugares desfasados."
    Write-Host "   Para reemplazar textos viejos usá -From <version>.`n"
}

Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Cyan
Write-Host "  🔖  Forja — Bump de versión" -ForegroundColor Cyan
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Cyan
Write-Host "   Anterior:  $OldVersion"
Write-Host "   Nueva:     $NewVersion"
Write-Host "   Modo git:  $GitMode`n"

$Count = 0

# Fija la versión del paquete en un Cargo.toml.
#
# Solo toca la PRIMERA `version = "…"` del archivo, la de `[package]`. Un
# reemplazo global también pisa dependencias declaradas como tabla multilínea
# (`[target.'cfg(target_os = "android")'.dependencies.winit]` + `version = …`)
# y deja el workspace sin resolver.
$ReVersionPaquete = [regex]'(?m)^version = "[0-9]+\.[0-9]+\.[0-9]+(?:-[a-zA-Z0-9.]+)?"'
function Set-VersionPaquete {
    param([string]$Texto)
    return $ReVersionPaquete.Replace($Texto, "version = `"$NewVersion`"", 1)
}

function Update-FileContent {
    param(
        [string]$RelativePath,
        [scriptblock]$ReplaceBlock
    )
    $fullPath = Join-Path $ProjectDir $RelativePath
    if (Test-Path $fullPath) {
        $content = Get-Content $fullPath -Raw
        $newContent = & $ReplaceBlock $content
        if ($content -ne $newContent) {
            # Sin BOM: `-Encoding UTF8` en PowerShell 5.1 agrega BOM y algunos
            # parseadores de Cargo.toml/JSON lo rechazan.
            [System.IO.File]::WriteAllText($fullPath, $newContent, (New-Object System.Text.UTF8Encoding($false)))
            Write-Host "   ✅ $RelativePath" -ForegroundColor Green
            $script:Count++
        }
    }
}

# 1. Cargo.toml raíz
# La versión se fija sin mirar la vieja: así el script repara Cargo.toml que
# quedaron atrás en un bump anterior.
Update-FileContent "Cargo.toml" {
    param($txt)
    Set-VersionPaquete $txt
}

# 2. Workspace crates
# La lista sale de [workspace] members + default-members de Cargo.toml: una
# lista fija acá se desfasó y dejó crates sueltos en la versión vieja.
foreach ($dir in $WorkspaceCrates) {
    $rel = "$dir\Cargo.toml"
    Update-FileContent $rel {
        param($txt)
        # Versión del paquete (solo la primera del archivo).
        $out = Set-VersionPaquete $txt
        # Dependencias internas del workspace: forja-x = { version = "…", path = "…" }.
        # Solo claves forja-*, para no pisar versiones de crates externos.
        $out -replace '(?m)^(forja-[a-z0-9-]+ *= *\{ *version *= *)"[0-9]+\.[0-9]+\.[0-9]+(?:-[a-zA-Z0-9.]+)?"', "`${1}`"$NewVersion`""
    }

    # README del crate: también lleva la versión en prosa.
    Update-FileContent "$dir\README.md" {
        param($txt)
        $txt -replace [regex]::Escape($OldVersion), $NewVersion
    }
}

# 3. .github/workflows/cd.yml
Update-FileContent ".github\workflows\cd.yml" {
    param($txt)
    $txt -replace "tag_name: v$([regex]::Escape($OldVersion))", "tag_name: v$NewVersion" `
         -replace "name: v$([regex]::Escape($OldVersion))", "name: v$NewVersion"
}

# 4. src/main.rs (templates de proyecto)
# Solo las dependencias `forja* = { version = … }`: el archivo también contiene
# versiones de otras dependencias que no deben tocarse.
Update-FileContent "src\main.rs" {
    param($txt)
    $txt -replace '(forja[a-z-]* *= *\{ *version *= *)"[0-9]+\.[0-9]+\.[0-9]+(?:-[a-zA-Z0-9.]+)?"', "`${1}`"$NewVersion`""
}

# 5. README.md (cualquier mención `vX.Y.Z`, no solo `Forja vX.Y.Z`)
Update-FileContent "README.md" {
    param($txt)
    $txt -replace "v$([regex]::Escape($OldVersion))", "v$NewVersion"
}

# 6. Benchmarks
Update-FileContent "benchmarks\README.md" {
    param($txt)
    $txt -replace "Forja-$([regex]::Escape($OldVersion))", "Forja-$NewVersion"
}
Update-FileContent "benchmarks\RESULTADOS_BENCHMARK.md" {
    param($txt)
    $txt -replace "Forja v$([regex]::Escape($OldVersion))", "Forja v$NewVersion"
}

# 7. Docs: página del runtime Android, portada y roadmap.
#    El resto de las menciones a versiones viejas en docs son históricas
#    ("removida en 0.9.0") y no deben tocarse.
Update-FileContent "docs\src\pages\arquitectura\forja_android_rt.astro" {
    param($txt)
    $txt -replace "forja-android-rt-$([regex]::Escape($OldVersion))\.aar", "forja-android-rt-$NewVersion.aar"
}
foreach ($doc in @("docs\src\pages\index.astro", "docs\src\pages\roadmap.astro")) {
    Update-FileContent $doc {
        param($txt)
        $txt -replace "v$([regex]::Escape($OldVersion))", "v$NewVersion"
    }
}

# 7b. Plantilla Android: nombre del AAR enlazado por Gradle.
#     Sin esto el proyecto generado pide `forja-android-rt-0.9.5.aar` y no
#     enlaza el AAR del release.
Update-FileContent "library\android\android\template\app\build.gradle.kts" {
    param($txt)
    $txt -replace "forja-android-rt-[0-9]+\.[0-9]+\.[0-9]+\.aar", "forja-android-rt-$NewVersion.aar"
}

# 7c. Scripts de instalación: los ejemplos de uso llevan la versión.
foreach ($inst in @("scripts\install.sh", "scripts\install.ps1")) {
    Update-FileContent $inst {
        param($txt)
        $txt -replace "v$([regex]::Escape($OldVersion))", "v$NewVersion"
    }
}

# 8. Scripts build-aar
Update-FileContent "scripts\build-aar.sh" {
    param($txt)
    $txt -replace "build-aar\.sh $([regex]::Escape($OldVersion))", "build-aar.sh $NewVersion" `
         -replace "Version=`"$([regex]::Escape($OldVersion))`"", "Version=`"$NewVersion`""
}
Update-FileContent "scripts\build-aar.ps1" {
    param($txt)
    $txt -replace "Version = `"$([regex]::Escape($OldVersion))`"", "Version = `"$NewVersion`""
}

# 9. VSCode extension
Update-FileContent "vscode\README.md" {
    param($txt)
    $txt -replace "$([regex]::Escape($OldVersion))", "$NewVersion"
}
Update-FileContent "vscode\forja-syntax\package.json" {
    param($txt)
    $txt -replace "`"version`": `"$([regex]::Escape($OldVersion))`"", "`"version`": `"$NewVersion`""
}
Update-FileContent "vscode\forja-syntax\package-lock.json" {
    param($txt)
    $txt -replace "`"version`": `"$([regex]::Escape($OldVersion))`"", "`"version`": `"$NewVersion`""
}

# 10. installer.iss (Inno Setup)
Update-FileContent "installer.iss" {
    param($txt)
    $txt -replace "#define MyAppVersion `"$([regex]::Escape($OldVersion))`"", "#define MyAppVersion `"$NewVersion`""
}

# 10b. Crate de soporte de tests: no está en `[workspace] members`.
Update-FileContent "tests\harness\Cargo.toml" {
    param($txt)
    Set-VersionPaquete $txt
}

# 10c. Instalador WiX y bootstrapper .NET (versión en prosa).
foreach ($wxs in @("installer\Bundle.wxs", "installer\msi.wxs")) {
    Update-FileContent $wxs {
        param($txt)
        $txt -replace '-d Version=[0-9]+\.[0-9]+\.[0-9]+', "-d Version=$NewVersion"
    }
}
Update-FileContent "installer\bootstrapper\ForjaBA.csproj" {
    param($txt)
    $txt -replace '<Version>[0-9]+\.[0-9]+\.[0-9]+</Version>', "<Version>$NewVersion</Version>"
}

# 10d. Fórmula de Homebrew: la genera scripts/update-homebrew.sh (bash), que la
#      reescribe entera desde la plantilla. Los sha256 quedan pendientes porque
#      el release sale después del tag; se rellenan corriendo ese script sin
#      --sin-hashes (ver packaging/homebrew-tap/README.md).
if (Test-Path "packaging\homebrew-tap\Formula\forja.rb") {
    if (Get-Command bash -ErrorAction SilentlyContinue) {
        & bash scripts/update-homebrew.sh $NewVersion --sin-hashes | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Falló scripts/update-homebrew.sh" }
        Write-Host "  ✅  packaging/homebrew-tap/Formula/forja.rb" -ForegroundColor Green
    } else {
        Write-Host "  ⚠️   bash no disponible: la fórmula de Homebrew no se regeneró" -ForegroundColor Yellow
    }
}

# 11. Verificación: ningún Cargo.toml del workspace debe quedar desfasado.
#     Es la red que faltaba: un bump incompleto dejaba el workspace sin resolver
#     (`forja-gui-rt 0.9.6` exigiendo `forja-gui-core ^0.9.6` con 0.9.5).
$Desfasados = @()
$tomls = @("Cargo.toml") + ($WorkspaceCrates | ForEach-Object { "$_\Cargo.toml" })
foreach ($toml in $tomls) {
    $full = Join-Path $ProjectDir $toml
    if (-not (Test-Path $full)) { continue }
    $lines = Get-Content $full
    $pkg = $lines | Where-Object { $_ -match '^version = "' } | Select-Object -First 1
    if ($pkg -and $pkg -match '^version = "([^"]+)"') {
        if ($Matches[1] -ne $NewVersion) { $Desfasados += "$toml : $($Matches[1])" }
    }
    foreach ($linea in ($lines | Where-Object { $_ -match '^forja-[a-z0-9-]+ *= *\{ *version' })) {
        if ($linea -match '^(forja-[a-z0-9-]+) *= *\{ *version *= *"([^"]+)"') {
            if ($Matches[2] -ne $NewVersion) { $Desfasados += "$toml : $($Matches[1]) = $($Matches[2])" }
        }
    }
}
if ($Desfasados.Count -gt 0) {
    Write-Host "`n❌ Quedaron versiones desfasadas:" -ForegroundColor Red
    $Desfasados | ForEach-Object { Write-Host "   - $_" -ForegroundColor Red }
    Write-Host ""
    exit 1
}
Write-Host "   ✅ Verificación: todos los Cargo.toml del workspace en $NewVersion" -ForegroundColor Green

# 11b. La fórmula de Homebrew también tiene que quedar en la versión nueva.
#      Si `bash` no está o alguien movió el archivo, esto lo delata acá y no en
#      el tap publicado. Con placeholder de sha256 `brew install` igual falla:
#      los hashes se rellenan cuando la release existe.
$formula = "packaging\homebrew-tap\Formula\forja.rb"
if (Test-Path (Join-Path $ProjectDir $formula)) {
    if (-not (Select-String -Path (Join-Path $ProjectDir $formula) -Pattern "^  version `"$([regex]::Escape($NewVersion))`"$" -Quiet)) {
        Write-Host "`n❌ $formula no quedó en $NewVersion" -ForegroundColor Red
        Write-Host "   Corregí con: bash scripts/update-homebrew.sh $NewVersion --sin-hashes" -ForegroundColor Red
        exit 1
    }
    Write-Host "   ✅ Verificación: fórmula de Homebrew en $NewVersion (sha256 pendientes del release)" -ForegroundColor Green
}

# 12. Cargo check to update Cargo.lock files
if (Get-Command "cargo" -ErrorAction SilentlyContinue) {
    Write-Host "`n  ℹ️ Actualizando Cargo.lock con cargo check..." -ForegroundColor Cyan
    cargo check --quiet 2>$null
    if (Test-Path (Join-Path $ProjectDir "library\wasm")) {
        Push-Location (Join-Path $ProjectDir "library\wasm")
        cargo check --quiet 2>$null
        Pop-Location
    }
    Write-Host "   ✅ Cargo.lock actualizado" -ForegroundColor Green

    # El workspace tiene que resolver después del bump. Sin esto, una versión
    # mal escrita en una dependencia pasa desapercibida hasta el primer build.
    cargo metadata --no-deps --format-version 1 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "`n❌ El workspace no resuelve después del bump. Revisá los Cargo.toml." -ForegroundColor Red
        exit 1
    }
    Write-Host "   ✅ Workspace resuelto" -ForegroundColor Green
}

Write-Host "`n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host "  ✅  Versión actualizada: $OldVersion → $NewVersion" -ForegroundColor Green
Write-Host "  📂  Archivos modificados: $Count" -ForegroundColor Green
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`n" -ForegroundColor Green

# ── Git Operations ──
$DoCommit = $GitMode -in @("commit", "tag", "push", "all")
$DoTag    = $GitMode -in @("tag", "push", "all")
$DoPush   = $GitMode -in @("push", "all")

$CommitMsg = "chore: bump version to $NewVersion"
$TagName   = "v$NewVersion"
$TagMsg    = "Release v$NewVersion"

if ($DoCommit) {
    Write-Host "  ── Paso 1/3: git commit ──" -ForegroundColor Cyan
    git add -A
    $status = git status --porcelain
    if (-not $status) {
        Write-Host "  ⚠️ No hay cambios para commitear." -ForegroundColor Yellow
        $DoTag = $false
        $DoPush = $false
    } else {
        git commit -m $CommitMsg
        Write-Host "   ✅ Commit creado: $CommitMsg" -ForegroundColor Green
    }
}

if ($DoTag) {
    Write-Host "`n  ── Paso 2/3: git tag ──" -ForegroundColor Cyan
    $existingTag = git tag -l $TagName
    if ($existingTag) {
        Write-Host "  ⚠️ El tag '$TagName' ya existe." -ForegroundColor Yellow
    } else {
        git tag -a $TagName -m $TagMsg
        Write-Host "   ✅ Tag creado: $TagName" -ForegroundColor Green
    }
}

if ($DoPush) {
    Write-Host "`n  ── Paso 3/3: git push ──" -ForegroundColor Cyan
    git push origin
    Write-Host "   ✅ Commits enviados a origin" -ForegroundColor Green
    git push origin --tags
    Write-Host "   ✅ Tags enviados a origin" -ForegroundColor Green
}

Write-Host "`n🎉 Release v$NewVersion completado!`n" -ForegroundColor Green
