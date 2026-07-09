// Forja GUI Runtime — Signals/Streams reactivos
// Reemplaza HashMap<String, ValorGUI> con un sistema reactivo
// que permite actualización granular de widgets vía generación counters.
//
// Arquitectura:
//   Signal<T>      → valor observable con detección de cambios O(1)
//   VariableStore  → contenedor de signals nombradas
//   Stream<T>      → canal de eventos (para futuras extensiones)

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

// ─── Signal ──────────────────────────────────────────────────────────
//
/// Un valor reactivo con seguimiento de cambios mediante un contador
/// de generación atómico. Cada escritura incrementa la generación,
/// permitiendo a los consumidores detectar cambios sin comparar valores.
///
/// # Reactividad
/// Los widgets pueden leer `generation()` antes y después de una operación
/// para determinar si hubo cambios sin necesidad de comparar el valor.
///
/// # Thread Safety
/// `Signal<T>` es `Send + Sync` cuando `T: Send + Sync`.
#[derive(Debug)]
pub struct Signal<T> {
    inner: Arc<RwLock<SignalInner<T>>>,
}

#[derive(Debug)]
struct SignalInner<T> {
    value: T,
    generation: u64,
}

impl<T> Signal<T> {
    /// Crea una nueva señal con el valor inicial y generación 0.
    pub fn new(value: T) -> Self {
        Signal {
            inner: Arc::new(RwLock::new(SignalInner {
                value,
                generation: 0,
            })),
        }
    }

    /// Obtiene el valor actual (clonado) y su generación.
    /// Requiere `T: Clone`.
    pub fn get(&self) -> (T, u64)
    where
        T: Clone,
    {
        let guard = self.inner.read().expect("Signal::get lock poisoned");
        (guard.value.clone(), guard.generation)
    }

    /// Escribe un nuevo valor, incrementa la generación y la retorna.
    pub fn write(&self, value: T) -> u64 {
        let mut guard = self.inner.write().expect("Signal::write lock poisoned");
        guard.value = value;
        guard.generation += 1;
        guard.generation
    }

    /// Obtiene la generación actual (sin leer el valor).
    pub fn generation(&self) -> u64 {
        self.inner
            .read()
            .expect("Signal::generation lock poisoned")
            .generation
    }

    /// Reemplaza el valor y retorna el valor anterior junto con la nueva generación.
    pub fn replace(&self, value: T) -> (T, u64)
    where
        T: Clone,
    {
        let mut guard = self.inner.write().expect("Signal::replace lock poisoned");
        let old = guard.value.clone();
        guard.value = value;
        guard.generation += 1;
        (old, guard.generation)
    }

    /// Actualiza el valor mediante una función, incrementa generación.
    pub fn update<F>(&self, f: F) -> u64
    where
        F: FnOnce(&mut T),
    {
        let mut guard = self.inner.write().expect("Signal::update lock poisoned");
        f(&mut guard.value);
        guard.generation += 1;
        guard.generation
    }
}

impl<T: PartialEq> Signal<T> {
    /// Escribe solo si el nuevo valor es diferente al actual.
    /// Retorna `true` si hubo cambio (y se incrementó la generación).
    pub fn write_if_changed(&self, value: T) -> bool {
        let mut guard = self.inner.write().expect("Signal::write_if_changed lock poisoned");
        if guard.value != value {
            guard.value = value;
            guard.generation += 1;
            true
        } else {
            false
        }
    }
}

impl<T: Clone> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Signal {
            inner: Arc::clone(&self.inner),
        }
    }
}

// Safety: Signal<T> es Send + Sync cuando T: Send + Sync
// porque usa Arc internamente
unsafe impl<T: Send> Send for Signal<T> {}
unsafe impl<T: Send + Sync> Sync for Signal<T> {}

// ─── Stream ──────────────────────────────────────────────────────────
//
/// Un canal de eventos simple para comunicación reactiva.
/// Cada `emit()` envía un valor y notifica a los consumidores.
///
/// Placeholder para futura implementación con multicanal.
#[derive(Debug)]
pub struct Stream<T> {
    inner: Arc<RwLock<StreamInner<T>>>,
}

#[derive(Debug)]
struct StreamInner<T> {
    value: Option<T>,
    generation: u64,
}

impl<T> Stream<T> {
    /// Crea un nuevo stream vacío.
    pub fn new() -> Self {
        Stream {
            inner: Arc::new(RwLock::new(StreamInner {
                value: None,
                generation: 0,
            })),
        }
    }

    /// Emite un valor al stream, incrementando la generación.
    pub fn emit(&self, value: T) -> u64 {
        let mut guard = self.inner.write().expect("Stream::emit lock poisoned");
        guard.value = Some(value);
        guard.generation += 1;
        guard.generation
    }

    /// Obtiene el último valor emitido (si hay) y su generación.
    pub fn peek(&self) -> (Option<&T>, u64) {
        let guard = self.inner.read().expect("Stream::peek lock poisoned");
        // No podemos devolver una referencia al valor dentro del lock
        // porque el lock se dropearía al salir. Retornamos generación nomas.
        (None, guard.generation)
    }

    /// Obtiene la generación actual.
    pub fn generation(&self) -> u64 {
        self.inner
            .read()
            .expect("Stream::generation lock poisoned")
            .generation
    }
}

impl<T> Default for Stream<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for Stream<T> {
    fn clone(&self) -> Self {
        Stream {
            inner: Arc::clone(&self.inner),
        }
    }
}

unsafe impl<T: Send> Send for Stream<T> {}
unsafe impl<T: Send + Sync> Sync for Stream<T> {}

// ─── ValorReactivio (enum wrapper) ──────────────────────────────────
//
/// Un valor que puede ser Signal (reactivo) o fijo.
/// Útil para parámetros de widgets que pueden ser estáticos o dinámicos.
#[derive(Debug, Clone)]
pub enum ValorReactivio<T: Clone> {
    Fijo(T),
    Signal(Signal<T>),
}

impl<T: Clone> ValorReactivio<T> {
    /// Obtiene el valor actual y su generación (0 si es fijo).
    pub fn get(&self) -> (T, u64) {
        match self {
            ValorReactivio::Fijo(val) => (val.clone(), 0),
            ValorReactivio::Signal(sig) => sig.get(),
        }
    }

    /// Obtiene la generación actual (0 si es fijo).
    pub fn generation(&self) -> u64 {
        match self {
            ValorReactivio::Fijo(_) => 0,
            ValorReactivio::Signal(sig) => sig.generation(),
        }
    }

    /// Crea un ValorReactivio desde un valor fijo.
    pub fn fijo(val: T) -> Self {
        ValorReactivio::Fijo(val)
    }

    /// Crea un ValorReactivio desde una signal.
    pub fn reactivo(signal: Signal<T>) -> Self {
        ValorReactivio::Signal(signal)
    }
}

impl<T: Clone> From<T> for ValorReactivio<T> {
    fn from(val: T) -> Self {
        ValorReactivio::Fijo(val)
    }
}

impl<T: Clone> From<Signal<T>> for ValorReactivio<T> {
    fn from(signal: Signal<T>) -> Self {
        ValorReactivio::Signal(signal)
    }
}

// ─── VariableStore ───────────────────────────────────────────────────
//
/// Reemplaza `HashMap<String, ValorGUI>` con un mapa de señales reactivas.
///
/// Cada variable es una `Signal<serde_json::Value>` independiente.
/// Los widgets pueden leer el valor y su generación para decidir
/// si necesitan re-renderizarse.
///
/// # Reactividad
/// ```ignore
/// let store = VariableStore::new();
/// store.set("contador", json!(42));
/// let gen = store.generation("contador");
/// // ... algo cambia ...
/// if store.generation("contador") != gen {
///     // el valor cambió, hay que re-renderizar
/// }
/// ```
///
/// # Thread Safety
/// `VariableStore` es `Send + Sync`.
#[derive(Debug, Clone)]
pub struct VariableStore {
    signals: Arc<RwLock<HashMap<String, Signal<serde_json::Value>>>>,
    /// Generación global que se incrementa en cada escritura.
    /// Útil para invalidación completa del árbol.
    global_gen: Arc<AtomicU64>,
}

impl VariableStore {
    /// Crea un nuevo VariableStore vacío.
    pub fn new() -> Self {
        VariableStore {
            signals: Arc::new(RwLock::new(HashMap::new())),
            global_gen: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Obtiene o crea la señal para una variable.
    fn ensure_signal(&self, name: &str) -> Signal<serde_json::Value> {
        let mut map = self.signals.write().expect("VariableStore::ensure_signal lock poisoned");
        map.entry(name.to_string())
            .or_insert_with(|| Signal::new(serde_json::Value::Null))
            .clone()
    }

    /// Obtiene la señal para una variable, si existe.
    fn get_signal(&self, name: &str) -> Option<Signal<serde_json::Value>> {
        let map = self.signals.read().expect("VariableStore::get_signal lock poisoned");
        map.get(name).cloned()
    }

    /// Lee el valor de una variable.
    /// Si no existe, retorna `None`.
    pub fn get(&self, name: &str) -> Option<serde_json::Value> {
        self.get_signal(name).map(|sig| sig.get().0)
    }

    /// Escribe un valor en una variable, incrementa su generación
    /// y la generación global. Retorna la generación de la señal.
    pub fn set(&self, name: &str, value: serde_json::Value) -> u64 {
        let signal = self.ensure_signal(name);
        let gen = signal.write(value);
        self.global_gen.fetch_add(1, Ordering::Release);
        gen
    }

    /// Obtiene la generación de una variable específica.
    /// Si no existe, retorna 0.
    pub fn generation(&self, name: &str) -> u64 {
        self.get_signal(name)
            .map(|sig| sig.generation())
            .unwrap_or(0)
    }

    /// Obtiene la generación global (se incrementa en cada escritura).
    pub fn global_generation(&self) -> u64 {
        self.global_gen.load(Ordering::Acquire)
    }

    /// Verifica si una variable existe.
    pub fn contains(&self, name: &str) -> bool {
        let map = self.signals.read().expect("VariableStore::contains lock poisoned");
        map.contains_key(name)
    }

    /// Obtiene el número de variables almacenadas.
    pub fn len(&self) -> usize {
        let map = self.signals.read().expect("VariableStore::len lock poisoned");
        map.len()
    }

    /// Retorna true si no hay variables.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Obtiene un snapshot de todas las variables (para depuración).
    pub fn snapshot(&self) -> HashMap<String, serde_json::Value> {
        let map = self.signals.read().expect("VariableStore::snapshot lock poisoned");
        map.iter()
            .map(|(k, sig)| (k.clone(), sig.get().0))
            .collect()
    }

    /// Inicializa múltiples variables desde un iterador.
    pub fn init_from<I: IntoIterator<Item = (String, serde_json::Value)>>(&self, iter: I) {
        let mut map = self.signals.write().expect("VariableStore::init_from lock poisoned");
        for (name, value) in iter {
            let entry = map
                .entry(name)
                .or_insert_with(|| Signal::new(serde_json::Value::Null));
            entry.write(value);
        }
    }
}

impl Default for VariableStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─── ReactiveContext ─────────────────────────────────────────────────
//
/// Contexto reactivo para el árbol de widgets.
///
/// Mantiene un registro de qué señales se leyeron durante el render actual,
/// permitiendo al sistema optimizar las actualizaciones.
#[derive(Debug, Clone)]
pub struct ReactiveCtx {
    /// Variables leídas durante el último render
    dependencies: Arc<RwLock<Vec<String>>>,
}

impl ReactiveCtx {
    /// Crea un nuevo contexto reactivo.
    pub fn new() -> Self {
        ReactiveCtx {
            dependencies: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Registra que una variable fue leída.
    pub fn track_dependency(&self, name: &str) {
        if let Ok(mut deps) = self.dependencies.write() {
            if !deps.contains(&name.to_string()) {
                deps.push(name.to_string());
            }
        }
    }

    /// Obtiene las dependencias registradas.
    pub fn get_dependencies(&self) -> Vec<String> {
        self.dependencies
            .read()
            .map(|deps| deps.clone())
            .unwrap_or_default()
    }

    /// Limpia las dependencias registradas.
    pub fn clear(&self) {
        if let Ok(mut deps) = self.dependencies.write() {
            deps.clear();
        }
    }
}

impl Default for ReactiveCtx {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Reactive Operations (helpers para widgets) ─────────────────────
//
/// Lee un valor reactivo con tracking de dependencia.
pub fn read_var(store: &VariableStore, name: &str, _ctx: Option<&ReactiveCtx>) -> Option<serde_json::Value> {
    if let Some(ctx) = _ctx {
        ctx.track_dependency(name);
    }
    store.get(name)
}

/// Escribe un valor reactivo.
pub fn write_var(store: &VariableStore, name: &str, value: serde_json::Value) -> u64 {
    store.set(name, value)
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_signal_basic() {
        let sig = Signal::new(42i64);
        assert_eq!(sig.generation(), 0);

        let (_, gen) = sig.get();
        assert_eq!(gen, 0);

        let new_gen = sig.write(100);
        assert!(new_gen > 0);
        assert_eq!(sig.generation(), new_gen);

        let (val, _) = sig.get();
        assert_eq!(val, 100);
    }

    #[test]
    fn test_signal_write_if_changed() {
        let sig = Signal::new(42i64);
        assert!(!sig.write_if_changed(42)); // mismo valor
        assert_eq!(sig.generation(), 0);

        assert!(sig.write_if_changed(100)); // valor diferente
        assert_eq!(sig.generation(), 1);
    }

    #[test]
    fn test_signal_update() {
        let sig = Signal::new(vec![1, 2, 3]);
        sig.update(|v| v.push(4));
        assert_eq!(sig.get().0, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_signal_thread_safety() {
        let sig = Signal::new(0i64);
        let sig2 = sig.clone();

        std::thread::spawn(move || {
            sig2.write(42);
        })
        .join()
        .unwrap();

        assert_eq!(sig.get().0, 42);
    }

    #[test]
    fn test_variable_store() {
        let store = VariableStore::new();

        // set/get
        store.set("nombre", json!("Juan"));
        assert_eq!(store.get("nombre"), Some(json!("Juan")));

        // generation
        let gen1 = store.generation("nombre");
        assert!(gen1 > 0);

        store.set("nombre", json!("María"));
        let gen2 = store.generation("nombre");
        assert!(gen2 > gen1);

        // contains
        assert!(store.contains("nombre"));
        assert!(!store.contains("no_existe"));

        // non-existent
        assert_eq!(store.get("no_existe"), None);
        assert_eq!(store.generation("no_existe"), 0);
    }

    #[test]
    fn test_variable_store_init() {
        let store = VariableStore::new();
        let mut data = HashMap::new();
        data.insert("a".to_string(), json!(1));
        data.insert("b".to_string(), json!("hola"));

        store.init_from(data.clone().into_iter());
        assert_eq!(store.get("a"), data.get("a").cloned());
        assert_eq!(store.get("b"), data.get("b").cloned());
    }

    #[test]
    fn test_valor_reactivio() {
        let sig = Signal::new(42i64);
        let fijo: ValorReactivio<i64> = ValorReactivio::Fijo(10);
        let reactivo: ValorReactivio<i64> = ValorReactivio::Signal(sig);

        assert_eq!(fijo.get(), (10, 0));
        assert_eq!(reactivo.generation(), 0);

        if let ValorReactivio::Signal(s) = &reactivo {
            s.write(99);
        }
        assert_eq!(reactivo.get().0, 99);
    }

    #[test]
    fn test_variable_store_thread_safety() {
        let store = VariableStore::new();
        let store_clone = store.clone();

        let handle = std::thread::spawn(move || {
            store_clone.set("x", json!(42));
        });

        handle.join().unwrap();
        assert_eq!(store.get("x"), Some(json!(42)));
    }

    #[test]
    fn test_store_snapshot_and_len() {
        let store = VariableStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store.set("x", json!(1));
        store.set("y", json!(2));

        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());

        let snap = store.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.get("x"), Some(&json!(1)));
    }

    #[test]
    fn test_signal_clone_independent_gen() {
        let sig = Signal::new("original".to_string());
        let cloned = sig.clone();

        // cloned ve el mismo valor inicial
        assert_eq!(cloned.get().0, "original".to_string());

        // escribir en original incrementa la gen para todos
        sig.write("modificado".to_string());
        assert_eq!(cloned.get().0, "modificado".to_string());

        // generation se comparte porque apunta al mismo Arc
        assert_eq!(sig.generation(), cloned.generation());
    }

    #[test]
    fn test_global_generation() {
        let store = VariableStore::new();
        let g0 = store.global_generation();

        store.set("a", json!(1));
        assert!(store.global_generation() > g0);

        let g1 = store.global_generation();
        store.set("b", json!(2));
        assert!(store.global_generation() > g1);
    }

    #[test]
    fn test_stream_basic() {
        let stream = Stream::new();
        assert_eq!(stream.generation(), 0);

        stream.emit("hola");
        assert!(stream.generation() > 0);

        stream.emit("mundo");
        assert!(stream.generation() > 0);
    }
}
