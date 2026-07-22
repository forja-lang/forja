# Changelog

Todas las versiones notables de **Forja (fa)** serán documentadas en este archivo.

Formato basado en [Keep a Changelog](https://keepachangelog.com/es/1.1.0/),
y este proyecto adhiere a [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.8] - 2025-??

### Agregado
- Soporte de diseño por contrato (`requiere` / `asegura`) en funciones
- Inicialización struct-literal con llaves: `nuevo Persona { nombre: "Ana", edad: 25 }`
- Operador ternario: `condicion ? valor_si : valor_no`
- Interpolación de strings con `${}`
- Acceso a mapas con sintaxis de punto: `config.host`
- Métodos integrados en tipos primitivos (`.longitud()`, `.a_mayusculas()`, etc.)
- Compilador al vuelo (JIT) nativo x86-64 con Direct Threading
- Máquina virtual ForjaFast con NaN tagging
- Compilación cruzada para Android (ARM64, x86_64, ARM32, x86)
- Soporte de módulos con hot-reload
- Sistema de paquetes (`forja add`, `forja remove`, `forja install`)
- Atributos `@test` y `@derive`
- Transpilación a Rust
- Generación de ensamblador nativo (x86-64 y ARM64)
- Interfaz gráfica con Material Design 3
- Soporte WASM (core + GUI)
- Servidor de lenguaje (LSP) y protocolo de depuración (DAP)

### Cambiado
- Optimizaciones de rendimiento en VM ForjaFast (NaN tagging)
- Mejoras en el sistema de ownership y préstamos
- Actualización a Rust edition 2021

### Corregido
- Múltiples correcciones en el parser y generación de bytecode
- Correcciones en el manejo de errores y panic en Android

## [0.8.7] - 2025-??

### Agregado
- Primer soporte de compilación JIT experimental
- Integración básica con Android NDK

### Cambiado
- Refactorización del sistema de tipos
- Mejoras en el mensajero de errores

### Corregido
- Correcciones en el lexer para cadenas multilínea
- Correcciones en el módulo de concurrencia

## [0.8.6] - 2025-??

### Agregado
- Palabras clave en español completas
- Sistema de clases y herencia
- Soporte de `importar` para módulos
- Canal de comunicación (`canal`, `enviar`, `recibir`, `unir`)
- Pattern matching (`coincidir` / `caso`)

### Cambiado
- Mejoras en la máquina virtual original
- Documentación extendida

## [0.8.5] - 2025-??

### Agregado
- Primer release público del compilador
- Ejecución en máquina virtual
- Variables, tipos, condicionales, bucles, funciones
- Operaciones matemáticas básicas
- Lectura y escritura en consola

---

El formato de versionado sigue el esquema `MAJOR.MINOR.PATCH`:

- **MAJOR**: Cambios incompatibles en el lenguaje o en el formato de bytecode
- **MINOR**: Nuevas funcionalidades compatibles hacia atrás
- **PATCH**: Correcciones de errores compatibles hacia atrás
