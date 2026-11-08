#!/usr/bin/env bash
# Genera los manifiestos de Scoop y winget para un release de Forja.
#
# Uso: scripts/generar-manifiestos.sh <version> [dir-dist]
#
# Escribe en el dir-dist:
#   forja-<version>.scoop.json   manifiesto de Scoop (listo para un bucket)
#   winget/ForjaLang.Forja.yaml                        (version)
#   winget/ForjaLang.Forja.installer.yaml              (installer)
#   winget/ForjaLang.Forja.locale.es-ES.yaml           (locale)
#
# Los manifiestos de winget se suben con un PR a microsoft/winget-pkgs; esto
# solo los deja listos y con el hash correcto.
set -euo pipefail

VERSION="${1:?falta la versión, ej: 0.9.7}"
DIST="${2:-dist}"
REPO="forja-lang/forja"
ZIP="forja-${VERSION}-windows-x64.zip"
SETUP="forja-setup-x64.exe"
BASE="https://github.com/${REPO}/releases/download/v${VERSION}"

mkdir -p "${DIST}/winget"

ZIP_HASH=""
SETUP_HASH=""
if [ -f "${DIST}/${ZIP}" ]; then
  ZIP_HASH=$(sha256sum "${DIST}/${ZIP}" | cut -d' ' -f1)
fi
if [ -f "${DIST}/${SETUP}" ]; then
  SETUP_HASH=$(sha256sum "${DIST}/${SETUP}" | cut -d' ' -f1)
fi

cat > "${DIST}/forja-${VERSION}.scoop.json" <<JSON
{
  "version": "${VERSION}",
  "description": "Forja (fa): lenguaje de programación educativo en español, con VM, LSP y DAP propios.",
  "homepage": "https://github.com/${REPO}",
  "license": "GPL-3.0-or-later",
  "architecture": {
    "64bit": {
      "url": "${BASE}/${ZIP}",
      "hash": "${ZIP_HASH}"
    }
  },
  "bin": ["forja.exe", "forja-lsp.exe", "forja-dap.exe", "forja-rt.exe"],
  "checkver": { "github": "https://github.com/${REPO}" },
  "autoupdate": {
    "architecture": {
      "64bit": { "url": "https://github.com/${REPO}/releases/download/v\$version/${ZIP//${VERSION}/\$version}" }
    }
  }
}
JSON

cat > "${DIST}/winget/ForjaLang.Forja.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.1.6.0.schema.json
PackageIdentifier: ForjaLang.Forja
PackageVersion: ${VERSION}
DefaultLocale: es-ES
ManifestType: version
ManifestVersion: 1.6.0
YAML

cat > "${DIST}/winget/ForjaLang.Forja.installer.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.1.6.0.schema.json
PackageIdentifier: ForjaLang.Forja
PackageVersion: ${VERSION}
InstallerLocale: es-ES
InstallerType: burn
Scope: user
InstallModes:
  - interactive
  - silent
  - silentWithProgress
InstallerSwitches:
  Silent: /install /quiet
  SilentWithProgress: /install /passive
  InstallLocation: InstallFolder="<INSTALLPATH>"
UpgradeBehavior: install
Commands:
  - forja
  - forja-lsp
  - forja-dap
  - forja-rt
ReleaseDate: $(date -u +%Y-%m-%d)
Installers:
  - Architecture: x64
    InstallerUrl: ${BASE}/${SETUP}
    InstallerSha256: ${SETUP_HASH^^}
    ProductCode: ForjaLang.Forja
ManifestType: installer
ManifestVersion: 1.6.0
YAML

cat > "${DIST}/winget/ForjaLang.Forja.locale.es-ES.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.locale.1.6.0.schema.json
PackageIdentifier: ForjaLang.Forja
PackageVersion: ${VERSION}
PackageLocale: es-ES
Publisher: Forja Lang
PublisherUrl: https://github.com/${REPO}
PublisherSupportUrl: https://github.com/${REPO}/issues
PackageName: Forja
PackageUrl: https://github.com/${REPO}
License: GPL-3.0-or-later
LicenseUrl: https://github.com/${REPO}/blob/main/LICENSE.md
ShortDescription: Lenguaje de programación educativo en español con VM, LSP y DAP propios.
Description: Forja (fa) incluye compilador, VM, servidor de lenguaje (LSP), depurador (DAP), runtime, biblioteca estándar y ejemplos.
Moniker: forja
Tags:
  - lenguaje
  - compilador
  - educativo
  - spanish
ManifestType: locale
ManifestVersion: 1.6.0
YAML

echo "✅ Manifiestos generados en ${DIST}:"
echo "   forja-${VERSION}.scoop.json"
echo "   winget/ForjaLang.Forja{,.installer,.locale.es-ES}.yaml"
