#!/bin/sh
# ─────────────────────────────────────────────────────────────────────────────
# Instalador de Forja (Linux / macOS) — F3-T7
#
# Descarga el paquete precompilado de la última release (o de una versión
# concreta), verifica su SHA-256 y deja los binarios en el prefijo elegido.
# NO requiere Rust ni compilar.
#
# Uso:
#   curl -fsSL https://raw.githubusercontent.com/forja-lang/forja/main/scripts/install.sh | sh
#   sh install.sh --version v0.9.5 --prefix "$HOME/.forja" --bin-dir "$HOME/.local/bin"
#
# Variables de entorno equivalentes: FORJA_VERSION, FORJA_PREFIX, FORJA_BIN_DIR.
# ─────────────────────────────────────────────────────────────────────────────
set -eu

REPO="${FORJA_REPO:-forja-lang/forja}"
VERSION="${FORJA_VERSION:-latest}"
PREFIX="${FORJA_PREFIX:-$HOME/.forja}"
BIN_DIR="${FORJA_BIN_DIR:-$HOME/.local/bin}"

usage() {
    cat <<EOF
Instalador de Forja

Opciones:
  --version <vX.Y.Z|latest>  Versión a instalar (por defecto: latest)
  --prefix <dir>            Directorio de instalación (por defecto: \$HOME/.forja)
  --bin-dir <dir>           Directorio de enlaces/binarios (por defecto: \$HOME/.local/bin)
  -h, --help                Muestra esta ayuda
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --prefix)  PREFIX="$2";  shift 2 ;;
        --bin-dir) BIN_DIR="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "❌ Opción desconocida: $1"; usage; exit 1 ;;
    esac
done

need() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "❌ Falta el comando '$1'. Instalalo y volvé a intentar."
        exit 1
    fi
}
need curl
need tar
need uname

OS=$(uname -s)
ARCH=$(uname -m)
case "$OS" in
    Linux)  PLAT="linux" ;;
    Darwin) PLAT="macos" ;;
    *) echo "❌ SO no soportado: $OS. En Windows usá scripts/install.ps1"; exit 1 ;;
esac
case "$ARCH" in
    x86_64|amd64)   A="x64" ;;
    arm64|aarch64)  A="arm64" ;;
    *) echo "❌ Arquitectura no soportada: $ARCH"; exit 1 ;;
esac
NAME="${PLAT}-${A}"

# ── Resolver la versión/tag ──
if [ "$VERSION" = "latest" ]; then
    TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep -o '"tag_name": *"[^"]*"' | head -1 \
        | sed 's/.*"tag_name": *"//; s/"//')
    [ -n "$TAG" ] || { echo "❌ No se pudo determinar la última versión en GitHub."; exit 1; }
else
    TAG="$VERSION"
    case "$TAG" in v*) ;; *) TAG="v$TAG" ;; esac
fi
VER="${TAG#v}"
ASSET="forja-${VER}-${NAME}.tar.gz"
BASE="https://github.com/$REPO/releases/download/$TAG"

echo "🔥 Instalando Forja $TAG ($NAME) en $PREFIX"

TMP=$(mktemp -d 2>/dev/null || mktemp -d -t forja)
trap 'rm -rf "$TMP"' EXIT INT TERM

echo "   ⬇️  Descargando $ASSET ..."
curl -fsSL "$BASE/$ASSET" -o "$TMP/$ASSET" || {
    echo "❌ No se pudo descargar $BASE/$ASSET"
    echo "   Verificá que exista la release y que '$NAME' sea correcto."
    exit 1
}

# ── Verificar SHA-256 (si hay SHA256SUMS en la release) ──
if curl -fsSL "$BASE/SHA256SUMS" -o "$TMP/SHA256SUMS" 2>/dev/null; then
    EXPECTED=$(grep " $ASSET\$" "$TMP/SHA256SUMS" | head -1 | awk '{print $1}')
    if [ -n "$EXPECTED" ]; then
        if command -v sha256sum >/dev/null 2>&1; then
            ACTUAL=$(sha256sum "$TMP/$ASSET" | awk '{print $1}')
        else
            ACTUAL=$(shasum -a 256 "$TMP/$ASSET" | awk '{print $1}')
        fi
        if [ "$EXPECTED" != "$ACTUAL" ]; then
            echo "❌ Checksum inválido para $ASSET"
            echo "   esperado: $EXPECTED"
            echo "   obtenido: $ACTUAL"
            exit 1
        fi
        echo "   ✅ Checksum verificado"
    fi
else
    echo "   ⚠️  SHA256SUMS no disponible; se omite la verificación."
fi

# ── F6-T7: verificar firma cosign (si el usuario tiene cosign) ──
if command -v cosign >/dev/null 2>&1; then
    if curl -fsSL "$BASE/$ASSET.bundle" -o "$TMP/$ASSET.bundle" 2>/dev/null; then
        if cosign verify-blob \
            --bundle "$TMP/$ASSET.bundle" \
            --certificate-identity-regexp "https://github.com/$REPO/.github/workflows/(rust|cd).yml@refs/heads/(main|master)" \
            --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
            "$TMP/$ASSET" >/dev/null 2>&1; then
            echo "   ✅ Firma cosign verificada"
        else
            echo "❌ Firma cosign inválida para $ASSET"
            exit 1
        fi
    else
        echo "   ⚠️  $ASSET.bundle no disponible; se omite la verificación de firma."
    fi
else
    echo "   ℹ️  cosign no instalado; se omite la verificación de firma (ver SECURITY.md)."
fi

# ── Extraer e instalar ──
tar -xzf "$TMP/$ASSET" -C "$TMP"
PKGDIR="$TMP/forja-${VER}-${NAME}"
[ -d "$PKGDIR" ] || { echo "❌ El paquete no tiene la estructura esperada."; exit 1; }

mkdir -p "$PREFIX" "$BIN_DIR"
cp -R "$PKGDIR"/. "$PREFIX"/

for b in forja forja-lsp forja-dap forja-rt forja-rt-gui; do
    if [ -f "$PREFIX/$b" ]; then
        chmod +x "$PREFIX/$b"
        ln -sf "$PREFIX/$b" "$BIN_DIR/$b"
    fi
done

echo "✅ Forja instalado en $PREFIX"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        echo "⚠️  Agregá '$BIN_DIR' al PATH (por ejemplo, añadilo a ~/.profile):"
        echo "      export PATH=\"$BIN_DIR:\$PATH\""
        ;;
esac
echo
"$BIN_DIR/forja" --version 2>/dev/null || "$PREFIX/forja" --version 2>/dev/null || true
echo "🎉 ¡Listo! Probá: forja run ejemplos/01_hola.fa"
