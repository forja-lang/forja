#!/bin/bash
# scripts/bump-version.sh
#
# 🔖 Script para actualizar la versión de Forja en todos los archivos relevantes
#     y opcionalmente hacer commit, tag y push automáticamente.
#
# Uso:
#   ./scripts/bump-version.sh 0.9.0              # Solo actualiza archivos
#   ./scripts/bump-version.sh 0.9.0 --commit     # Actualiza + commit
#   ./scripts/bump-version.sh 0.9.0 --tag        # Actualiza + commit + tag
#   ./scripts/bump-version.sh 0.9.0 --push       # Actualiza + commit + tag + push
#   ./scripts/bump-version.sh 0.9.0 --all        # Actualiza + commit + tag + push
#
# Requisitos:
#   - Bash 4+ (Linux, macOS, WSL, o Git Bash en Windows)
#   - git, sed
#
# Archivos que modifica:
#   - Cargo.toml (raíz)
#   - crates/forja-rt/Cargo.toml
#   - crates/forja-wasm/Cargo.toml
#   - crates/forja-android-rt/Cargo.toml
#   - .github/workflows/rust.yml (tag_name y name)
#   - src/main.rs (templates de proyecto nuevo)
#
# NOTA: Los crates forja-gui-rt y forja-wasm-gui mantienen su propia
#       versión independiente (no se sincronizan con el compilador).

set -euo pipefail

# ─── Funciones auxiliares ───────────────────────────────────────────────────
print_ok()   { echo -e "   \033[32m✅\033[0m $1"; }
print_info() { echo -e "  \033[36mℹ️\033[0m  $1"; }
print_warn() { echo -e "  \033[33m⚠️\033[0m  $1"; }

# ─── Parseo de argumentos ───────────────────────────────────────────────────
if [ $# -lt 1 ]; then
    echo "Uso: $0 <nueva_version> [--commit|--tag|--push|--all]"
    echo "Ejemplo: $0 0.9.0 --all"
    exit 1
fi

NEW_VERSION="$1"
GIT_MODE="${2:-none}"

# Validar modo git
case "$GIT_MODE" in
    none|--commit|--tag|--push|--all) ;;
    *)
        echo "❌ Error: Modo git inválido '$GIT_MODE'. Usar: --commit, --tag, --push o --all"
        exit 1
        ;;
esac

# ─── Validar formato semver (MAJOR.MINOR.PATCH o MAJOR.MINOR.PATCH-suffix) ──
if ! echo "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
    echo "❌ Error: '$NEW_VERSION' no es una versión semver válida (esperado: X.Y.Z)"
    exit 1
fi

# ─── Verificar que estamos en el root del repo ─────────────────────────────
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Este script debe ejecutarse desde la raíz del repositorio"
    exit 1
fi

# ─── Verificar que no hay cambios sin commitear (si vamos a hacer git) ────
if [ "$GIT_MODE" != "none" ]; then
    if ! git diff --quiet --exit-code 2>/dev/null; then
        echo "⚠️  Hay cambios sin commitear en el working directory."
        echo "   Se hará un commit de todo lo modificado."
    fi
fi

# ─── Detectar versión actual ───────────────────────────────────────────────
OLD_VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)

if [ -z "$OLD_VERSION" ]; then
    echo "❌ Error: No se pudo detectar la versión actual en Cargo.toml"
    exit 1
fi

if [ "$OLD_VERSION" = "$NEW_VERSION" ]; then
    echo "⚠️  La versión actual ya es $NEW_VERSION. No hay cambios que hacer."
    exit 0
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  🔖  Forja — Bump de versión"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "   Anterior:  $OLD_VERSION"
echo "   Nueva:     $NEW_VERSION"
echo "   Modo git:  ${GIT_MODE:-ninguno}"
echo ""

# ─── Actualizar archivos ───────────────────────────────────────────────────
COUNT=0

# 1. Cargo.toml raíz
sed -i "s/^version = \"$OLD_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
print_ok "Cargo.toml"
COUNT=$((COUNT + 1))

# 2. Workspace crates que comparten versión
for crate in forja-rt forja-wasm forja-android-rt; do
    file="crates/$crate/Cargo.toml"
    if [ -f "$file" ]; then
        sed -i "s/^version = \"$OLD_VERSION\"/version = \"$NEW_VERSION\"/" "$file"
        print_ok "$file"
        COUNT=$((COUNT + 1))
    fi
done

# 3. .github/workflows/rust.yml
ci_file=".github/workflows/rust.yml"
if [ -f "$ci_file" ]; then
    sed -i "s/tag_name: v$OLD_VERSION/tag_name: v$NEW_VERSION/" "$ci_file"
    sed -i "s/name: v$OLD_VERSION/name: v$NEW_VERSION/" "$ci_file"
    print_ok "$ci_file (tag_name y name)"
    COUNT=$((COUNT + 1))
fi

# 4. src/main.rs (templates de proyecto)
rs_file="src/main.rs"
if [ -f "$rs_file" ]; then
    sed -i "s/version = \"$OLD_VERSION\"/version = \"$NEW_VERSION\"/g" "$rs_file"
    print_ok "$rs_file (templates)"
    COUNT=$((COUNT + 1))
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ✅  Versión actualizada: $OLD_VERSION → $NEW_VERSION"
echo "  📂  Archivos modificados: $COUNT"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# ─── Operaciones git ────────────────────────────────────────────────────────

DO_COMMIT=false
DO_TAG=false
DO_PUSH=false

case "$GIT_MODE" in
    --commit) DO_COMMIT=true ;;
    --tag)    DO_COMMIT=true; DO_TAG=true ;;
    --push)   DO_COMMIT=true; DO_TAG=true; DO_PUSH=true ;;
    --all)    DO_COMMIT=true; DO_TAG=true; DO_PUSH=true ;;
esac

COMMIT_MSG="chore: bump version to $NEW_VERSION"
TAG_NAME="v$NEW_VERSION"
TAG_MSG="Release v$NEW_VERSION"

# ─── Commit ────────────────────────────────────────────────────────────────
if [ "$DO_COMMIT" = true ]; then
    echo "  ── Paso 1/3: git commit ──"
    git add -A

    # Verificar si hay algo para commitear
    if git diff --cached --quiet --exit-code 2>/dev/null; then
        print_warn "No hay cambios para commitear. Omitiendo."
        DO_TAG=false
        DO_PUSH=false
    else
        git commit -m "$COMMIT_MSG"
        print_ok "Commit creado: $COMMIT_MSG"
    fi
    echo ""
fi

# ─── Tag ───────────────────────────────────────────────────────────────────
if [ "$DO_TAG" = true ]; then
    echo "  ── Paso 2/3: git tag ──"

    # Verificar si el tag ya existe
    if git tag -l "$TAG_NAME" | grep -q .; then
        print_warn "El tag '$TAG_NAME' ya existe. Se omite creación."
    else
        git tag -a "$TAG_NAME" -m "$TAG_MSG"
        print_ok "Tag creado: $TAG_NAME"
    fi
    echo ""
fi

# ─── Push ──────────────────────────────────────────────────────────────────
if [ "$DO_PUSH" = true ]; then
    echo "  ── Paso 3/3: git push ──"
    echo ""

    # Primero mostrar qué se va a pushear
    print_info "Push de commits a origin..."
    git push origin
    print_ok "Commits enviados a origin"

    print_info "Push de tags a origin..."
    git push origin --tags
    print_ok "Tags enviados a origin"

    echo ""
fi

# ─── Resumen final ─────────────────────────────────────────────────────────
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  🎉  Release v$NEW_VERSION completado"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  CI/CD generará el release automáticamente en:"
echo "  https://github.com/$(git config --get remote.origin.url 2>/dev/null | sed 's/.*:\(.*\)\.git/\1/')/releases"
echo ""

if [ "$DO_PUSH" = false ]; then
    echo "📋  Para finalizar manualmente:"
    echo ""
    if [ "$DO_COMMIT" = false ]; then
        echo "     git add -A && git commit -m \"$COMMIT_MSG\""
    fi
    if [ "$DO_TAG" = false ]; then
        echo "     git tag -a $TAG_NAME -m \"$TAG_MSG\""
    fi
    echo "     git push && git push --tags"
    echo ""
fi

if [ "$DO_PUSH" = true ]; then
    echo "⚠️  Recordatorio: Si hay submódulos con cambios,"
    echo "   actualizarlos con: git submodule update --remote"
    echo ""
fi
