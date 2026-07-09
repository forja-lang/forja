# Parches para compatibilidad Android en Xilem

## ¿Por qué existen estos parches?

Xilem 0.4 (framework UI reactivo que usa Forja para GUI nativa) **sí soporta Android en su arquitectura**: el event loop usa `winit` que tiene soporte Android vía `android-activity`, el renderer `vello` es agnóstico de plataforma, y `wgpu` funciona en Android vía Vulkan/GLES. Sin embargo, hay **2 bloqueos de compilación** causados por configuraciones de `Cargo.toml` en sub-dependencias que asumen desktop:

1. **`masonry_winit`**: fuerza la feature `accesskit_unix` (solo Linux) en vez de seleccionar `accesskit_android` en Android.
2. **`xilem`**: pone la feature `android-native-activity` de `winit` solo en `dev-dependencies` (para ejemplos), no en `dependencies` (para consumers como `forja-gui-rt`).

copypasta (clipboard) **no necesita parche**: ya usa `NopClipboardContext` en Android, que es un stub funcional.

## Estructura

```
patches/
  xilem/            # Fork mínimo de xilem-0.4.0
  masonry_winit/    # Fork mínimo de masonry_winit-0.4.0
  README.md         # Este archivo
```

Ambos parches se referencian desde el `Cargo.toml` raíz vía `[patch.crates-io]`.

## Parches aplicados

### 1. `xilem/Cargo.toml`

**Problema**: `android-native-activity` solo disponible para dev-dependencies.

**Fix**: Agregar `[target.'cfg(target_os = "android")'.dependencies.winit]` con feature `android-native-activity`. Esto permite que `forja-gui-rt` (y cualquier consumer) herede la feature necesaria para que winit compile en Android.

**Antes:**
```toml
[target.'cfg(target_os = "android")'.dev-dependencies.winit]
features = ["android-native-activity"]
```

**Después:**
```toml
[target.'cfg(target_os = "android")'.dependencies.winit]
features = ["android-native-activity"]

[target.'cfg(target_os = "android")'.dev-dependencies.winit]
features = ["android-native-activity"]
```

### 2. `masonry_winit/Cargo.toml`

**Problema**: `accesskit_unix` feature hardcodeada, pero esa feature solo existe en Linux.

**Fix**: Dividir la dependencia de `accesskit_winit` en dos bloques target:

**Antes:**
```toml
[dependencies.accesskit_winit]
version = "0.29.2"
features = ["accesskit_unix", "tokio", "rwh_06"]
```

**Después:**
```toml
[target.'cfg(not(target_os = "android"))'.dependencies.accesskit_winit]
version = "0.29.2"
features = ["accesskit_unix", "tokio", "rwh_06"]

[target.'cfg(target_os = "android")'.dependencies.accesskit_winit]
version = "0.29.2"
features = ["accesskit_android", "tokio", "rwh_06"]
```

## ¿Cómo actualizar cuando salga Xilem 0.5+?

1. Copiar las fuentes nuevas a `patches/xilem` y `patches/masonry_winit`
2. Re-aplicar los parches de arriba (son ~5 líneas cada uno)
3. Verificar que `cargo check --target aarch64-linux-android` compile

## ¿Por qué no un fork real?

- Los cambios son mínimos (~7 líneas total)
- Usando `[patch.crates-io]` no necesitamos forks en GitHub mantenidos
- Cuando Xilem upstream acepte estos fixes (PRs), podemos eliminar los parches
- No hay vendor: las fuentes copiadas son pequeñas y obvias de mantener
