# App Android generada por Forja

Esta app fue generada automáticamente por el compilador Forja.

## ¿Cómo funciona?

1. El código fuente `.fa` está en `app/src/main/assets/main.fa`
2. Al compilar, `forja construir-apk` lo convierte a bytecode (`.fbc`)
3. La app Android carga y ejecuta el bytecode usando `forja-android-rt.so`
4. El output de `escribir()` se muestra en pantalla

## Uso

```bash
# Editar el código Forja
code app/src/main/assets/main.fa

# Compilar y generar APK
forja construir-apk

# O manualmente con Gradle
./gradlew assembleDebug
```
