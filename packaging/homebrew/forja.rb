# F3-T7 — Fórmula de Homebrew para Forja.
#
# Este archivo es una PLANTILLA. Los `sha256` se reemplazan ejecutando:
#
#     scripts/update-homebrew.sh [version]
#
# que descarga el `SHA256SUMS` de la release indicada y rellena los hashes.
# Para publicarla, copiala a un tap (p. ej. forja-lang/homebrew-tap) y usá:
#
#     brew tap forja-lang/tap
#     brew install forja
class Forja < Formula
  desc "Lenguaje de programación educativo en español, con contratos y tipado"
  homepage "https://github.com/forja-lang/forja"
  version "0.9.2"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/forja-lang/forja/releases/download/v0.9.2/forja-0.9.2-macos-arm64.tar.gz"
      sha256 "<sha256-macos-arm64>"
    end

    on_intel do
      url "https://github.com/forja-lang/forja/releases/download/v0.9.2/forja-0.9.2-macos-x64.tar.gz"
      sha256 "<sha256-macos-x64>"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/forja-lang/forja/releases/download/v0.9.2/forja-0.9.2-linux-arm64.tar.gz"
      sha256 "<sha256-linux-arm64>"
    end

    on_intel do
      url "https://github.com/forja-lang/forja/releases/download/v0.9.2/forja-0.9.2-linux-x64.tar.gz"
      sha256 "<sha256-linux-x64>"
    end
  end

  def install
    bin.install "forja", "forja-lsp", "forja-dap", "forja-rt", "forja-rt-gui"
    # La stdlib va embebida en el binario; se instala además como referencia
    # y para permitir actualizaciones manuales.
    prefix.install "stdlib" if Dir.exist?("stdlib")
    doc.install "README.md" if File.exist?("README.md")
  end

  test do
    assert_match "forja", shell_output("#{bin}/forja --version 2>&1").downcase
  end
end
