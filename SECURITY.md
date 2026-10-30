# Seguridad / Security Policy

## Versiones soportadas / Supported Versions

| Versión | Soporte de seguridad |
|---------|---------------------|
| 0.7.x   | ✅ Soportado |
| 0.6.x   | ⚠️ Soportado (solo critical) |
| < 0.6   | ❌ No soportado |

## Reportar vulnerabilidades / Reporting a Vulnerability

1. **No abras un issue público.** Contacta directamente a lococoi vía email o mensaje privado en GitHub.
2. Incluye: descripción, versión afectada, pasos para reproducir, impacto estimado.
3. Tiempo de respuesta: 7 días hábiles.

## Vulnerabilidades conocidas / Known Vulnerabilities

- **JIT native (x86-64)**: Por diseño, ejecuta código generado dinámicamente. No ejecutar código .fa de fuentes no confiables.
- **Channels/Threading**: El modelo de canales es seguro, pero seguimos auditorías.

## Prácticas de seguridad / Security Practices

- No se ejecuta código binario externo
- Las dependencias se auditan con `cargo audit`
- El código del usuario (programas .fa) corre aislado en la VM
- Los releases tienen artifact attestations generadas por GitHub Actions (Sigstore)

## Verificación de releases / Release Verification

Todos los releases publicados en GitHub incluyen **artifact attestations** firmadas criptográficamente mediante [GitHub Artifact Attestations](https://github.com/actions/attest-build-provenance) (basado en Sigstore) y, desde **F6-T7**, una firma **cosign keyless** por cada asset (`.bundle`).

### Verificar con cosign (recomendado)

```bash
cosign verify-blob \
  --bundle forja-<version>-<plataforma>.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/forja-lang/forja/.github/workflows/(rust|cd).yml@refs/heads/(main|master)' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  forja-<version>-<plataforma>.tar.gz
```

La **identidad pública** es el workflow oficial de entrega `forja-lang/forja/.github/workflows/cd.yml` en `main`/`master`, emitido por `token.actions.githubusercontent.com`. Los releases anteriores al split CI/CD (hasta `v0.9.5`) se firmaron con el workflow `rust.yml`, que ahora se llama `cd.yml`; el regex `(rust|cd)` cubre ambos. No hay una clave estática: la confianza se ancla en el OIDC de GitHub Actions y en el certificado de Sigstore. Los instaladores (`scripts/install.sh`, `scripts/install.ps1`) verifican automáticamente si `cosign` está disponible.

### Verificar con GitHub CLI

```bash
# Requiere GitHub CLI (gh) versión 2.0+
gh attestation verify <archivo> --owner forja-lang
```

Ejemplo:
```bash
gh attestation verify forja-linux --owner forja-lang
```

Si el artifact es íntegro y fue generado por el CI oficial de Forja, verás:
```
Loaded digest sha256:abc123... for forja-linux
Loaded 1 attestation from GitHub
...
✓ Verification succeeded!
```