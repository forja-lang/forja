#!/usr/bin/env bash
# scripts/firmar-release-local.sh — firma local de un artifact con cosign.
#
# Uso:
#   COSIGN_KEY=/ruta/cosign.key COSIGN_PASSWORD=... \
#     bash scripts/firmar-release-local.sh dist/forja-<v>-<target>.zip
#
# Genera `<artifact>.bundle` y verifica la firma con `cosign verify-blob`.
set -euo pipefail
ART="${1:?uso: $0 <artifact>}"
KEY="${COSIGN_KEY:?definí COSIGN_KEY}"
PUB="${COSIGN_PUB:-${KEY%.key}.pub}"

command -v cosign >/dev/null 2>&1 || { echo "❌ cosign no está instalado" >&2; exit 127; }
[ -f "$ART" ] || { echo "❌ no existe $ART" >&2; exit 1; }

cosign sign-blob --yes --key "$KEY" --bundle "$ART.bundle" "$ART"
cosign verify-blob --key "$PUB" --bundle "$ART.bundle" "$ART"
echo "✅ $ART firmado y verificado ($ART.bundle)"
