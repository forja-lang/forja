# forja-lang/homebrew-tap (scaffold local)

Tap de Homebrew para Forja. Este directorio es la estructura que se publica en
el repositorio `forja-lang/homebrew-tap`.

## Publicación

1. Generar un release firmado (`v<version>`), con `SHA256SUMS` y los tarballs
   `forja-<version>-{macos,linux}-{arm64,x64}.tar.gz`.
2. Actualizar la fórmula:

   ```bash
   scripts/update-homebrew.sh <version>
   ```

3. Copiar `formula/forja.rb` a `Formula/forja.rb` del tap y hacer commit.
4. Probar en macOS Intel/ARM64 y Linux:

   ```bash
   brew tap forja-lang/tap
   brew install forja
   forja --version
   ```

## Estado local

`Formula/forja.rb` es una copia de la plantilla; los `sha256` se completan
automáticamente con `scripts/update-homebrew.sh` al generar el release.
