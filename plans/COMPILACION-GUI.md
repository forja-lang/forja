# Tiempos de compilación de la GUI — análisis y decisiones

Fecha: 2026-11-16. Medido en Windows x64, `target/` caliente, rustc 1.100,
`RUSTC_WRAPPER=` (sccache falla con `os error 206` al compilar `forja-gui-rt`).

## Decisión de producto

**Las apps GUI son Forja, no Rust.** El `.fa` se ejecuta tal cual: `forja-rt-gui`
lo interpreta (`forja-interprete`) y dibuja con Xilem/wgpu. No se genera código
Rust a partir del programa del usuario.

`destino: nativo` (transpilar a Rust + `cargo build`) queda como **ruta
avanzada**, sólo para quien necesite un binario Rust propio o Android/web. No es
la forma de ejecutar ni de distribuir una app.

## Medición por app (lo que ve el usuario)

| Camino | Tiempo | ¿Compila el programa? |
|---|---|---|
| `forja compilar app.fa` (AOT, runtime prebuilt) | **47-55 ms** | no: incrusta el fuente |
| `forja ejecutar app.fa` (GUI, `destino: maquina`) | **340-420 ms** hasta que existe el proceso GUI | no (reusa el `.exe` si el `.fa` no cambió) |
| `destino: nativo` / `forja transpilar` (GUI) | **1m 11s** con caché, **8-15 min** en frío | sí: rustc + cargo por app |

Notas:
- `forja compilar` de GUI escribe 23 MB; si se repite sobre el mismo archivo, el
  antivirus lo escanea y cada corrida pasa a ~600 ms. Por eso `forja ejecutar`
  **reusa** el ejecutable temporal cuando el `.fa` y el runtime no cambiaron
  (`hay_que_recompilar_gui`).
- `forja version` = 12 ms (piso del arranque). Sin el caché de
  `update-check.json`, la consulta a GitHub bloqueaba **cada** comando ~0,5 s
  (y más con red lenta): ahora va en un hilo aparte.

## Costo de compilar el runtime (una vez por release/instalador)

`cargo build -p forja-rt --features gui --release` → **285 crates** (el CLI solo
usa 84). Con `--timings`, 427 unidades, 2018 s de CPU, 8m 14s de pared. Top:

| Unidad | s | Por qué |
|---|---:|---|
| `forja-rt` | 283 | **link + LTO fat + codegen-units=1** del binario de 23 MB |
| `naga` | 104 | traducción de shaders (wgpu) |
| `windows` | 84 | bindings de Win32 (wgpu/winit) |
| `forja-gui-rt` | 78 | el runtime (widgets + evaluador) |
| `libsqlite3-sys` | 77 | SQLite empaquetado (C) |
| `read-fonts` | 65 | parley/skrifa (texto) |
| `moxcms` | 56 | perfiles ICC de `image` |
| `ash` | 55 | Vulkan (wgpu) |
| `image` | 55 | formatos de imagen |
| `wgpu-hal` | 43 | backend de GPU |
| `rustls` | 43 | TLS de los nativos de red |

## Bucle de desarrollo (tocar `library/gui-rt` y reconstruir el runtime)

| Comando | Frío | Caliente |
|---|---|---|
| `cargo build -p forja-rt --features gui` (perfil `dev`) | 4m 00s | **21,6 s** |
| `cargo build -p forja-rt --profile rt --features gui` (calidad release) | 3m 36s | **113 s** |

Para iterar código de GUI, el perfil `dev` (21 s) es el que conviene; `--profile
rt` sólo para empaquetar o medir rendimiento.

## Mejoras aplicadas

0. **`[profile.rt]` (thin LTO, codegen-units=4)** para el runtime: **8m 14s →
   3m 37s** en frío, sin pérdida de frames. Es el mayor ahorro y aplica a cada
   rebuild de `forja-gui-rt` en desarrollo (`283 s` de link pasan a ~1/3).

1. **`image` sin AVIF** (`default-features = false`, png/jpeg/gif/bmp/webp/ico):
   saca del grafo a `rav1e`/`ravif`/`avif-serialize`. Build del runtime:
   **12m 11s → 8m 14s** (-33 %).
2. **`forja ejecutar` GUI reusa el `.exe`** si el `.fa` no cambió → ~350 ms de
   arranque en vez de reescribir 23 MB y pagar el antivirus.
3. **Chequeo de actualización en un hilo** (no bloquea ningún comando).
4. **`ejemplos/gui/forja.yaml`** vuelve a `destino: maquina` (los 27 ejemplos
   arrancan por el camino instantáneo, no con cargo).

## Próximos pasos propuestos (no aplicados)

Ordenados por relación beneficio/riesgo:

1. ~~Perfil dedicado para el runtime~~ **Aplicado**. `[profile.rt]` = release +
   `lto = "thin"` + `codegen-units = 4`. Medido: build en frío **3m 37s vs
   8m 14s** (-56 %); binario 24,8 MB vs 23,2 MB; frames idénticos
   (`render 1,2-2,4 ms`, misma serie con fat y thin). `cd.yml` y la doc del
   runtime ya compilan con `--profile rt` (el MSI sigue copiando desde
   `target/<triple>/release/forja-rt-gui.exe`, así que no cambió el empaquetado).
2. **Features de peso opcionales en `forja-gui-rt`** (`sqlite` = 77 s de C,
   `imagenes` = `image` + `moxcms` 111 s, TLS = 43 s, `qr`).
   **Decisión: no hacerlo ahora.** Requiere `#[cfg(feature = ...)]` en los
   módulos que usan esos crates (nativos `_sqlite_*`, `imagen()`, pickers con
   `rfd`, widget QR) con sus stubs; es cirugía amplia y riesgosa en el runtime
   que se distribuye. El beneficio hoy es chico: el bucle de desarrollo ya está
   en **22 s** y el build de distribución en **3m36s**. Si algún día molesta,
   el orden de corte por beneficio es: `image` (111 s) → `sqlite` (77 s) → TLS.
3. **Iteración de desarrollo**: usar perfil `dev` (opt-level 1 local + 3 en
   dependencias) → **2m 26s** en vez de 8-12 min; `--release` sólo para medir
   rendimiento o empaquetar.
4. **CI**: ya cachea con `swatinem/rust-cache`; conviene además compilar
   `forja-rt --features gui` sólo en el job de release (no en `workspace-tests`),
   o con `--no-default-features` allí.

## Cómo queda el camino a "Forja en Forja"

La app ya es 100 % Forja. Lo que hoy es Rust es la **capa de plataforma**
(ventana/GPU/widgets) más el evaluador. Cuando Forja se auto-hospede, esa capa
sigue siendo FFI (como Rust llama a C para gráficos): el `.fa` del usuario y el
runtime prebuilt no cambian, y no aparece Rust en el flujo de compilación de la
app.
