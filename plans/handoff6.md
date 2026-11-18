# Handoff 6 — GUI: runtime prebuilt, AOT y tiempos de compilación

Fecha: 2026-11-16. Repo: `C:\Users\gaucho\forja` (HEAD `377f5bd4`, árbol **sucio**).
Documentos hermanos: `plans/COMPILACION-GUI.md` (mediciones de tiempos),
`CHANGELOG.md` (entradas de la sesión), `plans/handoff2.md` (backend nativo).

> Este archivo es el mismo contenido que `plans/handoff4.md` (se pidieron los dos
> nombres); si sobra uno, borralo.

> `plans/` está en `.gitignore`: si querés versionar esto, `git add -f plans/`.

---

## 0. Estado en una frase

La GUI **funciona de punta a punta sin transpilar**: `forja compilar` y
`forja ejecutar` incrustan el **fuente Forja** (footer `FBG\0`) en el runtime
prebuilt `forja-rt-gui`, que lo interpreta (`forja-interprete`) y dibuja con
Xilem/wgpu. `destino: nativo` (transpilar a Rust + cargo) quedó como ruta
avanzada y **no es default en ningún lado**.

Números medidos (Windows x64, `RUSTC_WRAPPER=`):

| Qué | Tiempo |
|---|---|
| `forja compilar app.fa` (GUI) | **47-55 ms** |
| `forja ejecutar app.fa` (GUI) → proceso | **340-420 ms** (reusa el `.exe`) |
| Build del runtime, frío | **3m 36s** (`[profile.rt]`) |
| Build del runtime, tocando `gui-rt` | **22 s** (`dev`) / **113 s** (`rt`) |
| `destino: nativo` | 1m11s con caché, 8-15 min en frío |

---

## 1. Qué se tocó (y dónde)

### Repo principal (`forja`)

| Archivo | Cambio |
|---|---|
| `src/selfrun.rs` | payload **`FBG\0`** (fuente Forja) además de `FBC\0` (bytecode); `ejecutar_gui()` llama a `forja_gui_rt::gui_nativa::construir_y_ejecutar` con `--load-state` |
| `src/aot.rs` | para GUI el payload es el fuente; valida sintaxis antes de generar |
| `compiler/forja-driver/src/aot.rs` | magic por tipo de payload; **error claro si falta `forja-rt-gui`** (antes usaba `forja.exe` como stub y el `.exe` no dibujaba nada); test `magic_del_footer...` |
| `compiler/forja/Cargo.toml` | feature `gui = ["dep:forja-gui-rt"]` (opt-in). **No** está en `all` a propósito (si no, la CLI compila Xilem) |
| `compiler/forja-interprete/src/lib.rs` | nativo **`salir_app()`**: única forma de cerrar la ventana desde un callback (en wasm no hace nada) |
| `src/transpiler.rs` | emitía `forja_gui_rt::build_and_run[_android]` (**no existe**): ahora `gui_nativa::construir_y_ejecutar[_android]`; `#[allow(unused_imports)]` |
| `src/tools/forja-cli/src/commands/ejecucion.rs` | `forja ejecutar` GUI = AOT + run; `importa_gui()` compartido (`importar gui` / `std/gui`, ignora comentarios); temp estable `%TEMP%/forja_gui_run_<hash>` con **reuso por mtime**; `--locked`; `RUSTC_WRAPPER=`; `[patch]` en el proyecto de `destino: nativo` |
| `.../commands/consola.rs` | `forja transpilar`: manifiesto válido (`forja` en `compiler/forja`, antes apuntaba a la raíz virtual), `[patch]`, target dir compartido, sin `src/programa.json` muerto |
| `src/package_config.rs` | `forja` = versión real (`CARGO_PKG_VERSION`), `modulos: []` |
| `compiler/forja-session/src/lib.rs` | si el CWD no se puede escribir (`C:\Windows\system32`), `forja.yaml` va a `~/.forja/` |
| `src/actualizar.rs` | chequeo de versión **en un hilo** (antes frenaba cada comando ~0,5 s); el componente Runtime actualiza también `forja-rt-gui` |
| `Cargo.toml` (raíz) | `[patch."…/linebender/xilem"] masonry_winit` + **`[profile.rt]`** (thin LTO, cgu=4) |
| `installer/msi-comun.wxi`, `.github/workflows/cd.yml` | el MSI instala `forja-rt-gui`; el runtime se compila con `--profile rt` (copia desde `rt/`) |
| docs | `README.md`, `COMANDOS.md`, `CHANGELOG.md`, `patches/README.md`, `forja ayuda`, `plans/COMPILACION-GUI.md` |

### Submódulos — **hay que commitear en cada repo propio**

- `library/gui-rt`
  - `expr_a_layout` y `expr_a_valor`: **`Expresion::Resultado` se lee como la variable `resultado`**. Era el bug del contador: `resultado` es keyword
    (postcondiciones), la etiqueta quedaba vacía y parecía que los botones no hacían nada.
  - ventana **centrada** (`GetSystemMetrics` + `GetDpiForSystem`) y **traída al frente** (`EnumThreadWindows` + `SetForegroundWindow`, hilo a 400 ms).
  - `debug.rs`: `es_depuracion_enabled()` estaba **hardcodeado en `true`**; ahora `FORJA_GUI_DEBUG` / `FORJA_DEBUG` / `FORJA_GUI_PERF`.
  - `Cargo.toml`: `image` sin AVIF (saca `rav1e`).
  - `tests/estado_y_layout.rs`: regresión (label de `resultado` + callback que muta el store). **Corre en CI** (`cargo test --workspace`).
- `library/gui-core`: mismo debug hardcodeado, arreglado.
- `library/gui-theme`: `detect_system_theme()` cacheado (`OnceLock`) y `reg.exe` con `CREATE_NO_WINDOW` (eran las **dos consolas** que aparecían antes de la ventana).
- `library/rt`: `gui = ["forja/gui"]` (antes agregaba una dependencia que `main.rs` nunca usaba); README con el perfil `rt` y el gotcha de sccache.
- `library/gui`: README y `gui.fa` (ya no dicen "sólo transpilando a Rust" ni "`forja add gui`").
- `ejemplos`: `gui/forja.yaml` → `destino: maquina`; comentarios sin flags (`--native`, `forja-gui`); `205_gui_hola.fa` y `206_gui_contador.fa` reescritos con callbacks reales.

---

## 2. Pendiente / por mejorar (priorizado)

1. **Commitear todo.** Hecho para la GUI (sesión `01a1eadf`, 2026-11-17):
   `ce431808` (root: runtime prebuilt + AOT por fuente + feature `gui` +
   `[profile.rt]` + MSI/CD), `750de18a` (punteros), y commits en `gui-rt`,
   `gui-core`, `gui-theme`, `gui`, `rt` y `ejemplos/gui`. **Falta** lo de las
   otras sesiones (nativo `forja-native/*`, VM/HIR/bytecode/IR, golden tests,
   `src/lib.rs`, `src/monomorph.rs`): no es de este tema y no se tocó.
2. **Tests de interacción reales.** **Hecho en parte** (2026-11-17):
   `gui_nativa::tests_click` recorre el camino del click dentro del proceso
   (`MaterialButton` → `ejecutar_callback_y_actualizar` → store → re-extraer
   layout), con uno y dos clicks. `masonry_testing` **no aplica** en esta revisión
   de xilem (su `TestHarness` monta widgets de Masonry; el árbol de la app es una
   vista de Xilem y no hay forma pública de materializarla sin el driver). Lo que
   sigue sin cubrirse es el evento del sistema (mouse → acción).
3. **Hot reload para AOT GUI.** **Hecho** como *reinicio*: `recarga: verdadero`
   en `forja ejecutar` rearma y relanza el `.exe` cuando cambia un `.fa` del
   proyecto (el estado del store no sobrevive; para eso está `estado:`). La idea
   original (recargar dentro del proceso preservando el snapshot) queda abierta si
   algún día importa: habría que re-inicializar el driver de Masonry.
4. **`forja ejecutar` GUI no propaga `--load-state`.** **Hecho**: clave `estado:`
   en `forja.yaml` (ruta a un JSON); la CLI lo pasa como `--load-state=<json>`.
   Test en `gui-rt`: `el_estado_guardado_pisa_las_variables_de_main`.
5. **`importar` de módulos `.fa` en apps GUI.** **Hecho** (2026-11-17): el
   payload GUI es el AST resuelto en JSON (footer `FBA`) y el runtime lo
   deserializa; `FBG` (fuente) y `FBC` (bytecode) siguen soportados. Test:
   `aot::tests::el_payload_de_una_app_gui_trae_los_modulos_propios`.
6. **Features opcionales de `forja-gui-rt`** (`sqlite` 77 s, `image`+`moxcms`
   111 s, TLS 43 s, `qr`): **decisión tomada = no** por ahora (requiere
   `#[cfg(feature)]` en nativos y widgets, con stubs). Reabrir sólo si molesta;
   orden de corte en `plans/COMPILACION-GUI.md`.
7. **`patches`** — **hecho** (2026-11-17): `masonry_winit` resincronizado a xilem
   `9f504ae` + los dos cambios de Forja (verificado con `diff -r` contra el
   checkout de cargo: no sobra nada) y **pusheado** a
   `github.com/forja-lang/patches` (`564658e`). `patches` pasó a ser **submódulo**:
   el `[patch]` de la raíz apunta a `patches/masonry_winit`, que estaba en
   `.gitignore`, así que un clon limpio y el CI no compilaban la GUI.
   `patches/xilem`, `masonry` y `copypasta` siguen viejos y sin cablear (no se
   aplican).
8. **`library/wasm-gui` y `library/gui-dom`** verificados el 2026-11-17:
   `cargo check --target wasm32-unknown-unknown` en verde (antes no compilaban por
   `Valor::Nulo()` en `salir_app`). Falta probarlos en un navegador.
9. **Evaluador vs VM** — **Fases 0-3 de B16 hechas**: hay **un** intérprete
   (`forja-interprete`) para las dos GUIs; `gui-rt` y `wasm-gui` sólo dibujan y
   traducen eventos, y los nativos salen de `forja-native` (registro compartido
   con la VM). La paridad VM ↔ evaluador la fija `tests/paridad_vm.rs`
   (12 programas, sin divergencias conocidas). Queda la **Fase 4** (widgets como
   valores, B12/B15), que es otro plan; y la VM sigue aparte por diseño (D1).
10. **`gui_nativa.rs` (8,5k líneas)** — **hecho** (2026-11-17): partido por dominio
    en `gui_nativa.rs` (estado, probes de ancho, arranque, Android y tests,
    1,7k líneas), `gui_nativa/layout_nativo.rs` (enum `Layout` + `expr_a_layout` +
    `extraer_layout` + helpers, 4,1k) y `gui_nativa/vista_nativa.rs`
    (`layout_a_vista`, 2,8k). Movimiento mecánico con `pub use` en el padre: las
    rutas públicas no cambian.
11. **Android**: `construir_y_ejecutar_android` compila pero no se probó en
    dispositivo. `cargo check --target aarch64-linux-android` falla en este
    entorno por `libsqlite3-sys`/`ring` (necesitan NDK/cc configurado), no por el
    código Forja.
12. **CI**: ya está: `workspace-tests` instala las cabeceras de winit/wayland e
    incluye `forja-gui-rt` con `--workspace` a propósito (así corre
    `tests/estado_y_layout.rs`).
13. **`target/debug` pesaba 43 GB** — **borrado** (2026-11-17): el disco pasó de
    22 GB a 76 GB libres. Se reconstruyeron `target/debug/forja.exe` y
    `forja-rt-gui.exe` (los que usa la CLI). `target/rt` (1,5 GB) y el caché de
    cargo para GUI (`~/.cache/forja-gui-target`) siguen intactos para que
    `destino: nativo` no tarde 10 minutos.

---

## 3. Cómo verificar en 2 minutos

```powershell
cd C:\Users\gaucho\forja

# 1) compilar / ejecutar GUI (no debe abrir consola, ventana centrada al frente)
target\debug\forja.exe compilar ejemplos\gui\205_gui_hola.fa "$env:TEMP\hola.exe"
& "$env:TEMP\hola.exe"                      # ventana con textos y botón Cerrar

target\debug\forja.exe ejecutar ejemplos\gui\206_gui_contador.fa
                                            # 0 → +1 → 1 → -1 → 0 → Cerrar cierra

# 2) que el payload sea FUENTE Forja, no bytecode
python -c "d=open(r'$env:TEMP\hola.exe','rb').read(); print(d[-4:])"   # b'FBG\0'

# 3) runtime y tests
cargo build -p forja-rt --profile rt --features gui        # 3m36 frío / ~2 min caliente
cargo test -p forja-gui-rt --test estado_y_layout          # 2 passed
cargo test -p forja-cli                                    # 25 passed
cargo test -p forja-driver                                 # 8 passed

# 4) rendimiento de frames (fork de masonry_winit)
$env:FORJA_GUI_PERF=1; $env:FORJA_CONSOLA=1; & "$env:TEMP\hola.exe"
# → [frame] redraw 0.10 ms | render 2.3 ms
```

El runtime instalado para el CLI es `target\rt\forja-rt.exe` copiado a
`target\debug\forja-rt-gui.exe` (es el que `forja compilar` busca al lado del
compilador). Si falta, `forja compilar` falla con el comando exacto para
compilarlo.

---

## 4. Gotchas que cuestan horas

- **Otra sesión edita el repo al mismo tiempo.** Durante esta sesión rompió y
  arregló `forja-hir`/`src/lib.rs` varias veces. Si `cargo build` falla con
  errores de argumentos/tuplas, mirá `git diff` antes de tocar nada.
- **`target\debug\forja.exe` desaparece** de vez en cuando: es el
  auto-actualizador (shadow copy) de la otra sesión. Recompilá; no es un bug de build.
- **`resultado` es palabra reservada** (postcondiciones). Si escribís
  `escribir(resultado)` en una app GUI, el runtime lo trata como la variable;
  en la VM/bytecode es `Expresion::Resultado`.
- **sccache roto con GUI**: con `RUSTC_WRAPPER=sccache`, compilar `forja-gui-rt`
  falla con `os error 206` (línea de comandos demasiado larga). Usá
  `RUSTC_WRAPPER=` (la CLI ya lo hace al compilar apps GUI).
- **`FORJA_CONSOLA=1`** mantiene la consola (la app GUI es subsistema ventana);
  sin eso se hace `FreeConsole` y no ves los `eprintln!`.
- **`destino: nativo`** se dispara por el `forja.yaml` del directorio de
  ejecución: `ejemplos/gui/forja.yaml` debe quedar en `maquina` o los ejemplos
  tardan 10 min.
- `plans/` y `*.txt` están gitignored.

---

## 5. Decisiones abiertas (para el dueño del producto)

1. ¿Se mantiene `destino: nativo` (transpilar a Rust) o se elimina del todo?
   Hoy está documentado como avanzado y sólo lo usa `forja transpilar`.
   **Dato nuevo (2026-11-17)**: se verificó de punta a punta con una app que
   importa un módulo propio — `destino: nativo` en `forja.yaml` compila
   (`Finished release` ~25 s con caché) y abre la ventana. Para eso hubo que
   arreglar tres bugs del transpilador (constructor de clases vacío, tipo de
   campo ignorado, escapes `\n`/`\r`) y resolver los `importar`. Sin esos
   arreglos la ruta estaba muerta con cualquier programa que usara clases o la
   stdlib.
2. ¿`forja ejecutar` de una app GUI debe compilar al vuelo (como ahora) o ser un
   alias explícito de `forja compilar` + ejecutar?
3. ¿La distribución del runtime debe incluir SQLite/imágenes (hoy sí, encendido
   todo) aunque eso cueste ~3 min más de build?
4. ¿Cuándo se empieza la unificación evaluador↔VM (camino a Forja en Forja)?
