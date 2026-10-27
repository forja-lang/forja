#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# update-homebrew.sh — regenera packaging/homebrew/forja.rb con los SHA-256
# reales de una release de Forja (F3-T7).
#
# Uso:
#   scripts/update-homebrew.sh [version] [--sums <archivo-o-url>] [--repo owner/repo]
#
# Si no se indica versión, se toma de Cargo.toml. Si no se pasa --sums, se
# descarga el SHA256SUMS de la release v<version> en GitHub.
#
# El resultado reemplaza packaging/homebrew/forja.rb (la plantilla con los
# placeholders <sha256-...>).
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO="forja-lang/forja"
VERSION=""
SUMS=""

while [ $# -gt 0 ]; do
    case "$1" in
        --sums) SUMS="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        -h|--help)
            grep '^#' "$0" | sed 's/^# \{0,1\}//' | sed -n '1,20p'
            exit 0 ;;
        *)
            if [ -z "$VERSION" ]; then VERSION="$1"; shift; else
                echo "❌ Argumento inesperado: $1"; exit 1
            fi ;;
    esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [ -z "$VERSION" ]; then
    VERSION=$(grep '^version' "$ROOT/Cargo.toml" | head -1 | cut -d'"' -f2)
fi
VERSION="${VERSION#v}"

# ── Obtener SHA256SUMS ──
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
if [ -z "$SUMS" ]; then
    URL="https://github.com/$REPO/releases/download/v$VERSION/SHA256SUMS"
    echo "⬇️  Descargando $URL"
    curl -fsSL "$URL" -o "$TMP/SHA256SUMS"
    SUMS="$TMP/SHA256SUMS"
elif [ ! -f "$SUMS" ]; then
    echo "⬇️  Descargando $SUMS"
    curl -fsSL "$SUMS" -o "$TMP/SHA256SUMS"
    SUMS="$TMP/SHA256SUMS"
fi

sha_for() {
    local asset="forja-$VERSION-$1.tar.gz"
    local s
    s=$(grep " $asset\$" "$SUMS" | head -1 | awk '{print $1}')
    if [ -z "$s" ]; then
        echo "❌ No se encontró '$asset' en $SUMS" >&2
        exit 1
    fi
    printf '%s' "$s"
}

SHA_MAC_ARM=$(sha_for macos-arm64)
SHA_MAC_X64=$(sha_for macos-x64)
SHA_LIN_ARM=$(sha_for linux-arm64)
SHA_LIN_X64=$(sha_for linux-x64)

OUT="$ROOT/packaging/homebrew/forja.rb"
echo "📝 Generando $OUT para v$VERSION"

cat > "$OUT" <<EOF
# F3-T7 — Fórmula de Homebrew para Forja.
#
# GENERADO por scripts/update-homebrew.sh v$VERSION — no editar a mano.
# Publicar en un tap (p. ej. forja-lang/homebrew-tap):
#
#     brew tap forja-lang/tap
#     brew install forja
class Forja < Formula
  desc "Lenguaje de programación educativo en español, con contratos y tipado"
  homepage "https://github.com/forja-lang/forja"
  version "$VERSION"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/$REPO/releases/download/v$VERSION/forja-$VERSION-macos-arm64.tar.gz"
      sha256 "$SHA_MAC_ARM"
    end

    on_intel do
      url "https://github.com/$REPO/releases/download/v$VERSION/forja-$VERSION-macos-x64.tar.gz"
      sha256 "$SHA_MAC_X64"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/$REPO/releases/download/v$VERSION/forja-$VERSION-linux-arm64.tar.gz"
      sha256 "$SHA_LIN_ARM"
    end

    on_intel do
      url "https://github.com/$REPO/releases/download/v$VERSION/forja-$VERSION-linux-x64.tar.gz"
      sha256 "$SHA_LIN_X64"
    end
  end

  def install
    bin.install "forja", "forja-lsp", "forja-dap", "forja-rt", "forja-rt-gui"
    prefix.install "stdlib" if Dir.exist?("stdlib")
    doc.install "README.md" if File.exist?("README.md")
  end

  test do
    assert_match "forja", shell_output("#{bin}/forja --version 2>&1").downcase
  end
end
EOF

echo "✅ Fórmula actualizada."
