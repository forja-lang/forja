# Gobernanza de Forja (fa)

## 1. Licencia

Forja (compilador y herramientas) se distribuye bajo la
**GNU General Public License, versión 3 o posterior (GPL-3.0-or-later)**.
Esta es la **única** licencia declarada en el repositorio:

- [`.gitignore`](.gitignore) → [`Cargo.toml`](Cargo.toml:7) declara
  `license = "GPL-3.0-or-later"`.
- [`LICENSE.md`](LICENSE.md) contiene el texto completo de la GPLv3.
- [`README.md`](README.md:279) resume la licencia.

### Licencias de los programas escritos en Forja

Escribir software *en* Forja **no** obliga a licenciarlo bajo GPL: los programas
del usuario son de su propiedad y pueden distribuirse con la licencia que
prefieran. La GPL aplica al **compilador y sus herramientas**.

## 2. Modelo de gobernanza

Forja es un proyecto mantenido de forma centralizada, con apertura a
contribuciones externas.

- **Mantenedor principal.** Tiene la última palabra sobre decisiones de diseño,
  arquitectura y releases, y la responsabilidad de la dirección técnica.
- **Mantenedores.** Personas con permiso de merge sobre áreas concretas.
- **Contribuidores.** Cualquier persona que envíe issues o pull requests.

## 3. Proceso de decisión

1. **Propuesta.** Las ideas se discuten en un *issue* (o *discussion*).
2. **Implementación.** La propuesta aceptada se implementa en un PR.
3. **Revisión.** Al menos un mantenedor revisa el PR.
4. **Criterios de merge.**
   - El CI pasa en verde (build + test + lint).
   - Existe cobertura razonable de tests para el cambio.
   - El cambio respeta las decisiones de higiene descritas en `CONTRIBUTING.md`.
5. **Rupturas.** Los cambios incompatibles deben documentarse en
   [`CHANGELOG.md`](CHANGELOG.md) y, si aplica, en la documentación.

## 4. Releases

- El versionado sigue [Versionado Semántico](https://semver.org/lang/es/)
  (la versión se declara en [`Cargo.toml`](Cargo.toml:3)).
- Los artefactos de distribución se compilan con el perfil `dist`
  (ver [`CONTRIBUTING.md`](CONTRIBUTING.md)).
- El flujo de publicación está descrito en
  [`.github/workflows/rust.yml`](.github/workflows/rust.yml).

## 5. Marca

El nombre **«Forja»**, el logotipo y los recursos visuales del proyecto son de
uso identificativo del proyecto oficial. Puedes referirte a Forja por su nombre
(al estilo «compatible con Forja», «construido para Forja»), pero no debes
sugerir afiliación u origen oficial sin autorización del mantenedor.

## 6. Código de conducta

Toda participación se rige por el [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
