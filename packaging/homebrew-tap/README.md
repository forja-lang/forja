# forja-lang/homebrew-tap (scaffold local)

Estructura del tap de Homebrew de Forja, la que se publica en el repositorio
`forja-lang/homebrew-tap`.

`Formula/forja.rb` **es generado**: no se edita a mano.

| Cuándo | Comando |
|---|---|
| Durante un bump (la release todavía no existe) | `scripts/update-homebrew.sh <version> --sin-hashes` |
| Después de publicar el release | `scripts/update-homebrew.sh` |

`scripts/bump-version.sh` ya corre el primero: la versión de la fórmula sigue
a `compiler/forja/Cargo.toml` sin intervención manual, pero los `sha256` quedan
como placeholder y `brew install` falla hasta que se corre el segundo.

## Publicación

1. Generar un release firmado (`v<version>`) con `SHA256SUMS` y los tarballs
   `forja-<version>-{macos,linux}-{arm64,x64}.tar.gz`.
2. `scripts/update-homebrew.sh` → rellena los `sha256` reales.
3. Copiar `Formula/forja.rb` al tap `forja-lang/homebrew-tap` y hacer commit.
4. Probar en macOS Intel/ARM64 y Linux:

   ```bash
   brew tap forja-lang/tap
   brew install forja
   forja --version
   ```

## Layout del tarball

El `install` de la fórmula depende del paso "Empaquetar (Unix)" de
`.github/workflows/cd.yml`: un directorio `forja-<version>-<plataforma>/` con
`forja`, `forja-lsp`, `forja-dap`, `forja-rt`, `forja-rt-gui`, `stdlib/` y los
`.md`. Si ese empaquetado cambia, hay que actualizar
`scripts/update-homebrew.sh`, no la fórmula.
