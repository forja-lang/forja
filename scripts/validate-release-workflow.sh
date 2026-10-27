#!/usr/bin/env bash
# scripts/validate-release-workflow.sh — valida la configuración de release
# (F6-T7/F6-T8/F9-T3/F9-T4) sin depender de GitHub Actions.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WF="$ROOT/.github/workflows/rust.yml"

fail() { echo "❌ $1" >&2; exit 1; }
ok() { echo "✅ $1"; }

grep -q "windows-11-arm" "$WF" || fail "falta windows-arm64 en la matriz"
grep -q "aarch64-pc-windows-msvc" "$WF" || fail "falta el target aarch64-pc-windows-msvc"
ok "Windows ARM64 presente en la matriz"

grep -q "sigstore/cosign-installer" "$WF" || fail "falta cosign-installer"
grep -q "cosign sign-blob" "$WF" || fail "falta cosign sign-blob"
grep -q "attest-build-provenance" "$WF" || fail "falta artifact attestation"
ok "firma cosign y atestación configuradas"

for script in scripts/install.sh scripts/install.ps1; do
    [ -f "$ROOT/$script" ] || fail "falta $script"
done
grep -q "cosign verify-blob" "$ROOT/scripts/install.sh" || fail "install.sh no verifica cosign"
grep -qi "cosign verify-blob" "$ROOT/scripts/install.ps1" || fail "install.ps1 no verifica cosign"
ok "instaladores verifican cosign"

grep -q "SHA256SUMS" "$ROOT/scripts/install.sh" || fail "install.sh no verifica SHA256"
grep -q "SHA256SUMS" "$ROOT/scripts/install.ps1" || fail "install.ps1 no verifica SHA256"
ok "instaladores verifican SHA256SUMS"

bash -n "$ROOT/scripts/install.sh" || fail "install.sh tiene errores de sintaxis"
ok "install.sh sintaxis válida"

echo "✅ Workflow de release consistente"
