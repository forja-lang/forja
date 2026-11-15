# Fórmula de Homebrew para Forja.
#
# GENERADO por scripts/update-homebrew.sh v0.11.0 — no editar a mano.
# Publicar en el tap forja-lang/homebrew-tap (copiar este archivo a Formula/):
#
#     brew tap forja-lang/tap
#     brew install forja
class Forja < Formula
  desc "Lenguaje de programación educativo en español, con contratos y tipado"
  homepage "https://github.com/forja-lang/forja"
  version "0.11.0"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/forja-lang/forja/releases/download/v0.11.0/forja-0.11.0-macos-arm64.tar.gz"
      sha256 "<sha256-macos-arm64>"
    end

    on_intel do
      url "https://github.com/forja-lang/forja/releases/download/v0.11.0/forja-0.11.0-macos-x64.tar.gz"
      sha256 "<sha256-macos-x64>"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/forja-lang/forja/releases/download/v0.11.0/forja-0.11.0-linux-arm64.tar.gz"
      sha256 "<sha256-linux-arm64>"
    end

    on_intel do
      url "https://github.com/forja-lang/forja/releases/download/v0.11.0/forja-0.11.0-linux-x64.tar.gz"
      sha256 "<sha256-linux-x64>"
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
