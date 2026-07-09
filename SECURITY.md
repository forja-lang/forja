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