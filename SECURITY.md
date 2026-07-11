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

Todos los releases publicados en GitHub incluyen **artifact attestations** firmadas criptográficamente mediante [GitHub Artifact Attestations](https://github.com/actions/attest-build-provenance) (basado en Sigstore).

### Verificar un artifact descargado

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