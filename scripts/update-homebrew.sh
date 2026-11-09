#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# update-homebrew.sh — regenera packaging/homebrew-tap/Formula/forja.rb (la
# fórmula que se publica en el tap forja-lang/homebrew-tap).
#
# Uso:
#   scripts/update-homebrew.sh [version] [--sums <archivo-o-url>] [--repo owner/repo]
#   scripts/update-homebrew.sh [version] --sin-hashes
#
# Si no se indica versión, se toma de compiler/forja/Cargo.toml. Si no se pasa
# --sums, se descarga el SHA256SUMS de la release v<version> en GitHub.
#
# --sin-hashes: escribe la fórmula con la versión nueva y deja los `sha256`
# como placeholder. Se usa desde scripts/bump-version.sh, cuando la release
# todavía no existe (los tarballs los genera el CD después del tag). Una vez
# publicada la release, hay que volver a correr el script SIN el flag para
# rellenar los hashes; con placeholders `brew install` falla.
#
# El layout del tarball lo define .github/workflows/cd.yml (paso "Empaquetar
# (Unix)"): un directorio forja-<version>-<plataforma>/ con los binarios y
# stdlib/ al lado. Los `bin.install`/`prefix.install` de abajo dependen de eso.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO="forja-lang/forja"
VERSION=""
SUMS=""
SIN_HASHES=false

while [ $# -gt 0 ]; do
    case "$1" in
        --sums) SUMS="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        --sin-hashes) SIN_HASHES=true; shift ;;
        -h|--help)
            grep '^#' "$0" | sed 's/^# \{0,1\}//' | sed -n '1,22p'
            exit 0 ;;
        *)
            if [ -z "$VERSION" ]; then VERSION="$1"; shift; else
                echo "❌ Argumento inesperado: $1"; exit 1
            fi ;;
    esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [ -z "$VERSION" ]; then
    # Cargo.toml raíz es un workspace virtual: la versión vive en el crate principal.
    VERSION=$(awk -F'"' '/^version = /{print $2; exit}' "$ROOT/compiler/forja/Cargo.toml")
fi
VERSION="${VERSION#v}"

# ── Obtener SHA256SUMS (salvo --sin-hashes) ──
if [ "$SIN_HASHES" = true ]; then
    SHA_MAC_ARM="<sha256-macos-arm64>"
    SHA_MAC_X64="<sha256-macos-x64>"
    SHA_LIN_ARM="<sha256-linux-arm64>"
    SHA_LIN_X64="<sha256-linux-x64>"
else
    TMP=$(mktemp -d)
    trap 'rm -rf "$TMP"' EXIT
    if [ -z "$SUMS" ]; then
        URL="https://github.com/$REPO/releases/download/v$VERSION/SHA256SUMS"
        echo "⬇️  Descargando $URL"
        if ! curl -fsSL "$URL" -o "$TMP/SHA256SUMS"; then
            echo "❌ No existe la release v$VERSION todavía." >&2
            echo "   Para dejar la fórmula apuntando a esa versión antes del" >&2
            echo "   release, usá --sin-hashes (los sha256 quedan pendientes)." >&2
            exit 1
        fi
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
fi

OUT="$ROOT/packaging/homebrew-tap/Formula/forja.rb"
mkdir -p "$(dirname "$OUT")"
echo "📝 Generando $OUT para v$VERSION"

cat > "$OUT" <<EOF
# Fórmula de Homebrew para Forja.
#
# GENERADO por scripts/update-homebrew.sh v$VERSION — no editar a mano.
# Publicar en el tap forja-lang/homebrew-tap (copiar este archivo a Formula/):
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
    # Nombres y layout fijados por el paso "Empaquetar (Unix)" del CD.
    bin.install "forja", "forja-lsp", "forja-dap", "forja-rt", "forja-rt-gui"
    # La stdlib va embebida en el binario; se instala además como referencia.
    prefix.install "stdlib" if Dir.exist?("stdlib")
    doc.install Dir["*.md"]
  end

  def caveats
    "forja-rt-gui necesita Vulkan (Linux) o Metal (macOS); el resto funciona sin GPU."
  end

  test do
    assert_match "forja", shell_output("#{bin}/forja --version 2>&1").downcase
  end
end
EOF

echo "✅ Fórmula actualizada."
if [ "$SIN_HASHES" = true ]; then
    echo "⚠️  sha256 pendientes: corré este script sin --sin-hashes cuando la release esté publicada."
fi
