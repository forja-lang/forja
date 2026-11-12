// Runtime de soporte del backend LLVM de Forja.
//
// Se compila junto al `.ll` generado (clang -O3 main.ll forja_rt.c). Sólo expone
// primitivas sin estado global: texto (NUL-terminated, `char*`), arreglos y
// tiempo. Todo lo que asigna memoria con `forja_alloc` vive hasta el final del
// proceso (no hay GC en la ruta nativa todavía).
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef _WIN32
#include <windows.h>
#endif

/* ── memoria ──────────────────────────────────────────────────────────── */

/* Arena: la memoria de un programa Forja no se libera durante la corrida (la
 * ruta nativa no tiene GC), así que en vez de un `calloc` por objeto se reparte
 * desde bloques de 1 MB, alineado a 16 bytes (ABI). Los pedidos grandes van
 * directo a `calloc`. */
#define FORJA_ARENA_BLOQUE (1024 * 1024)
static char *arena_actual = NULL;
static int64_t arena_restante = 0;

void *forja_alloc(int64_t bytes) {
    int64_t n = bytes > 0 ? bytes : 1;
    n = (n + 15) & ~(int64_t)15;
    if (n > FORJA_ARENA_BLOQUE / 4) {
        void *p = calloc(1, (size_t)n);
        if (!p) {
            fputs("forja: sin memoria\n", stderr);
            exit(1);
        }
        return p;
    }
    if (n > arena_restante) {
        arena_actual = (char *)calloc(1, FORJA_ARENA_BLOQUE);
        if (!arena_actual) {
            fputs("forja: sin memoria\n", stderr);
            exit(1);
        }
        arena_restante = FORJA_ARENA_BLOQUE;
    }
    void *p = arena_actual;
    arena_actual += n;
    arena_restante -= n;
    return p;
}

/* ── tiempo ───────────────────────────────────────────────────────────── */

/* Alta resolución: `GetTickCount64` tiene ~15 ms de granularidad, inservible
 * para medir bucles cortos. */
int64_t forja_tiempo_ms(void) {
#ifdef _WIN32
    static LARGE_INTEGER frecuencia = {0};
    LARGE_INTEGER ahora;
    if (frecuencia.QuadPart == 0) {
        QueryPerformanceFrequency(&frecuencia);
    }
    QueryPerformanceCounter(&ahora);
    return (int64_t)((ahora.QuadPart * 1000) / frecuencia.QuadPart);
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
#endif
}

/* ── texto ────────────────────────────────────────────────────────────── */

static char *forja_dup(const char *s) {
    size_t n = strlen(s) + 1;
    char *r = (char *)forja_alloc((int64_t)n);
    memcpy(r, s, n);
    return r;
}

char *forja_concat(const char *a, const char *b) {
    if (!a) return forja_dup(b ? b : "nulo");
    if (!b) return forja_dup(a);
    size_t la = strlen(a), lb = strlen(b);
    char *r = (char *)forja_alloc((int64_t)(la + lb + 1));
    memcpy(r, a, la);
    memcpy(r + la, b, lb + 1);
    return r;
}

char *forja_a_texto(int64_t v) {
    char *r = (char *)forja_alloc(32);
    snprintf(r, 32, "%lld", (long long)v);
    return r;
}

char *forja_decimal_a_texto(double v) {
    char *r = (char *)forja_alloc(64);
    /* %g como la VM: 1.5 -> "1.5", 2.0 -> "2" */
    snprintf(r, 64, "%g", v);
    return r;
}

char *forja_texto_nulo(void) { return forja_dup("nulo"); }

int64_t forja_a_entero(const char *s) {
    if (!s) return 0;
    char *fin = NULL;
    long long v = strtoll(s, &fin, 10);
    if (fin == s) return 0;
    return (int64_t)v;
}

double forja_a_decimal(const char *s) {
    if (!s) return 0.0;
    return strtod(s, NULL);
}

int64_t forja_longitud(const char *s) { return s ? (int64_t)strlen(s) : 0; }

int64_t forja_indice_de(const char *s, const char *sub) {
    if (!s || !sub) return -1;
    const char *p = strstr(s, sub);
    return p ? (int64_t)(p - s) : -1;
}

char *forja_subcadena(const char *s, int64_t ini, int64_t fin) {
    if (!s) return forja_dup("");
    int64_t n = (int64_t)strlen(s);
    if (ini < 0) ini = 0;
    if (fin > n) fin = n;
    if (fin < ini) fin = ini;
    int64_t len = fin - ini;
    char *r = (char *)forja_alloc(len + 1);
    memcpy(r, s + ini, (size_t)len);
    r[len] = '\0';
    return r;
}

int64_t forja_texto_igual(const char *a, const char *b) {
    if (!a || !b) return a == b;
    return strcmp(a, b) == 0;
}

int64_t forja_texto_orden(const char *a, const char *b) {
    if (!a || !b) return a ? 1 : (b ? -1 : 0);
    return (int64_t)strcmp(a, b);
}

int64_t forja_contiene(const char *s, const char *sub) {
    return (s && sub && strstr(s, sub)) ? 1 : 0;
}

int64_t forja_empieza_con(const char *s, const char *pre) {
    if (!s || !pre) return 0;
    return strncmp(s, pre, strlen(pre)) == 0;
}

int64_t forja_termina_con(const char *s, const char *suf) {
    if (!s || !suf) return 0;
    size_t ls = strlen(s), lf = strlen(suf);
    if (lf > ls) return 0;
    return strcmp(s + ls - lf, suf) == 0;
}

char *forja_a_mayusculas(const char *s) {
    char *r = forja_dup(s ? s : "");
    for (char *p = r; *p; p++)
        if (*p >= 'a' && *p <= 'z') *p = (char)(*p - 32);
    return r;
}

char *forja_a_minusculas(const char *s) {
    char *r = forja_dup(s ? s : "");
    for (char *p = r; *p; p++)
        if (*p >= 'A' && *p <= 'Z') *p = (char)(*p + 32);
    return r;
}

/* ── impresión ────────────────────────────────────────────────────────── */

void forja_imprimir_texto(const char *s) { fputs(s ? s : "nulo", stdout); }
void forja_imprimir_entero(int64_t v) { printf("%lld", (long long)v); }
void forja_imprimir_decimal(double v) { printf("%g", v); }
void forja_imprimir_booleano(int64_t v) { fputs(v ? "verdadero" : "falso", stdout); }

char *forja_booleano_a_texto(int64_t v) {
    return forja_dup(v ? "verdadero" : "falso");
}
void forja_imprimir_nulo(void) { fputs("nulo", stdout); }
void forja_nueva_linea(void) { fputc('\n', stdout); }

/* ── arreglos ─────────────────────────────────────────────────────────── */
/* Los arreglos de Forja son heterogéneos: cada casilla guarda un i64 (los
 * textos viajan como puntero convertido con ptrtoint). */
typedef struct {
    int64_t len;
    int64_t cap;
    int64_t tipo; /* 0 = elementos enteros, 1 = elementos texto (puntero) */
    int64_t *datos;
} ForjaArreglo;

void *forja_arreglo_nuevo(int64_t n, int64_t tipo) {
    ForjaArreglo *a = (ForjaArreglo *)forja_alloc((int64_t)sizeof(ForjaArreglo));
    a->len = n;
    a->cap = n > 4 ? n : 4;
    a->tipo = tipo;
    a->datos = (int64_t *)forja_alloc(a->cap * (int64_t)sizeof(int64_t));
    return a;
}

/* `repetir_arreglo(valor, n)`: n copias de `valor` en un solo recorrido. */
void *forja_arreglo_rellenar(int64_t valor, int64_t n, int64_t tipo) {
    ForjaArreglo *a = (ForjaArreglo *)forja_arreglo_nuevo(n, tipo);
    for (int64_t i = 0; i < n; i++) {
        a->datos[i] = valor;
    }
    return a;
}

int64_t forja_arreglo_longitud(void *pv) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    return a ? a->len : 0;
}

static void forja_arreglo_crecer(ForjaArreglo *a, int64_t necesarios) {
    if (necesarios <= a->cap) return;
    int64_t cap = a->cap ? a->cap : 4;
    while (cap < necesarios) cap *= 2;
    int64_t *d = (int64_t *)forja_alloc(cap * (int64_t)sizeof(int64_t));
    if (a->datos) memcpy(d, a->datos, (size_t)(a->len * (int64_t)sizeof(int64_t)));
    a->datos = d;
    a->cap = cap;
}

void forja_empujar(void *pv, int64_t v);

int64_t forja_arreglo_obtener(void *pv, int64_t i) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    if (!a || i < 0 || i >= a->len) {
        fprintf(stderr, "forja: índice %lld fuera de rango (longitud %lld)\n", (long long)i,
                (long long)(a ? a->len : 0));
        exit(1);
    }
    return a->datos[i];
}

void forja_arreglo_fijar(void *pv, int64_t i, int64_t v) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    /* `arr[longitud(arr)] = v` es la forma de empujar de la VM: crece. */
    if (a && i == a->len) {
        forja_empujar(a, v);
        return;
    }
    if (!a || i < 0 || i >= a->len) {
        fprintf(stderr, "forja: índice %lld fuera de rango (longitud %lld)\n", (long long)i,
                (long long)(a ? a->len : 0));
        exit(1);
    }
    a->datos[i] = v;
}

/* Imprime un arreglo como `[1,2,3]` (texto según el tag del arreglo). */
void forja_imprimir_arreglo(void *pv) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    fputc('[', stdout);
    if (a) {
        for (int64_t i = 0; i < a->len; i++) {
            if (i) fputc(',', stdout);
            if (a->tipo == 1) {
                const char *s = (const char *)(size_t)a->datos[i];
                fputs(s ? s : "nulo", stdout);
            } else {
                printf("%lld", (long long)a->datos[i]);
            }
        }
    }
    fputc(']', stdout);
}

/* Repr de un arreglo como Texto (para concatenar): mismo formato que la
 * impresión y que la VM — `[64,34,90]` / `[a,b]`. */
char *forja_arreglo_a_texto(void *pv) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    if (!a) return forja_dup("nulo");
    int64_t n = 2;
    for (int64_t i = 0; i < a->len; i++) {
        if (a->tipo == 1) {
            const char *s = (const char *)(size_t)a->datos[i];
            n += (int64_t)strlen(s ? s : "nulo");
        } else {
            n += 24;
        }
        n += 1; /* separador */
    }
    char *r = (char *)forja_alloc(n + 1);
    char *p = r;
    *p++ = '[';
    for (int64_t i = 0; i < a->len; i++) {
        if (i) *p++ = ',';
        if (a->tipo == 1) {
            const char *s = (const char *)(size_t)a->datos[i];
            const char *t = s ? s : "nulo";
            size_t l = strlen(t);
            memcpy(p, t, l);
            p += l;
        } else {
            p += sprintf(p, "%lld", (long long)a->datos[i]);
        }
    }
    *p++ = ']';
    *p = '\0';
    return r;
}

/* Reserva capacidad para `n` elementos (no cambia la longitud). */
void forja_reservar(void *pv, int64_t n) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    if (!a || n <= a->cap) return;
    int64_t cap = a->cap ? a->cap : 4;
    while (cap < n) cap *= 2;
    int64_t *d = (int64_t *)forja_alloc(cap * (int64_t)sizeof(int64_t));
    if (a->datos) memcpy(d, a->datos, (size_t)(a->len * (int64_t)sizeof(int64_t)));
    a->datos = d;
    a->cap = cap;
}

void forja_empujar(void *pv, int64_t v) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    if (!a) return;
    forja_arreglo_crecer(a, a->len + 1);
    a->datos[a->len++] = v;
}

/* ── mapas ────────────────────────────────────────────────────────────── */
/* Tabla abierta (linear probing) con dos espacios de claves: enteras y texto.
 * `m[1]` y `m["1"]` son claves distintas (en la VM son la misma porque la clave
 * siempre es String). Las claves de texto se guardan por puntero (el texto vive
 * en el heap del programa, que no se libera). */
typedef struct {
    int64_t clave;
    const char *clave_texto;
    int64_t valor;
    int64_t usado;
    /* 1 si `valor` es un puntero a Texto (la casilla es un i64 sin tipo). */
    int64_t valor_texto;
} ForjaPar;

typedef struct {
    ForjaPar *tabla;
    int64_t cap; /* potencia de 2 */
    int64_t len;
} ForjaMapa;

static int64_t forja_hash_i64(int64_t x) {
    uint64_t h = (uint64_t)x * 0x9E3779B97F4A7C15ull;
    h ^= h >> 29;
    return (int64_t)h;
}

static int64_t forja_hash_texto(const char *s) {
    uint64_t h = 1469598103934665603ull;
    for (; s && *s; s++) {
        h ^= (unsigned char)*s;
        h *= 1099511628211ull;
    }
    return (int64_t)h;
}

void *forja_mapa_nuevo(void) {
    ForjaMapa *m = (ForjaMapa *)forja_alloc((int64_t)sizeof(ForjaMapa));
    m->cap = 16;
    m->len = 0;
    m->tabla = (ForjaPar *)forja_alloc(m->cap * (int64_t)sizeof(ForjaPar));
    return m;
}

int64_t forja_mapa_longitud(void *pm) {
    ForjaMapa *m = (ForjaMapa *)pm;
    return m ? m->len : 0;
}

static ForjaPar *forja_mapa_slot(ForjaMapa *m, int64_t hash, int64_t clave, const char *texto) {
    int64_t i = (int64_t)((uint64_t)hash & (uint64_t)(m->cap - 1));
    for (;;) {
        ForjaPar *p = &m->tabla[i];
        if (!p->usado) return p;
        if (clave >= 0 && p->clave_texto == NULL && p->clave == clave) return p;
        if (texto && p->clave_texto && strcmp(p->clave_texto, texto) == 0) return p;
        i = (i + 1) & (m->cap - 1);
    }
}

static void forja_mapa_crecer(ForjaMapa *m) {
    ForjaPar *vieja = m->tabla;
    int64_t vieja_cap = m->cap;
    m->cap *= 2;
    m->tabla = (ForjaPar *)forja_alloc(m->cap * (int64_t)sizeof(ForjaPar));
    m->len = 0;
    for (int64_t i = 0; i < vieja_cap; i++) {
        if (!vieja[i].usado) continue;
        ForjaPar *d = forja_mapa_slot(m, vieja[i].clave_texto ? forja_hash_texto(vieja[i].clave_texto)
                                                            : forja_hash_i64(vieja[i].clave),
                                      vieja[i].clave_texto ? -1 : vieja[i].clave, vieja[i].clave_texto);
        *d = vieja[i];
        m->len++;
    }
}

void forja_mapa_fijar_i64(void *pm, int64_t clave, int64_t valor, int64_t es_texto) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m) return;
    if ((m->len + 1) * 2 > m->cap) forja_mapa_crecer(m);
    ForjaPar *p = forja_mapa_slot(m, forja_hash_i64(clave), clave, NULL);
    if (!p->usado) {
        p->usado = 1;
        p->clave = clave;
        p->clave_texto = NULL;
        m->len++;
    }
    p->valor = valor;
    p->valor_texto = es_texto;
}

int64_t forja_mapa_obtener_i64(void *pm, int64_t clave) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m || m->len == 0) return 0;
    ForjaPar *p = forja_mapa_slot(m, forja_hash_i64(clave), clave, NULL);
    return p->usado ? p->valor : 0;
}

void forja_mapa_fijar_texto(void *pm, const char *clave, int64_t valor, int64_t es_texto) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m) return;
    if ((m->len + 1) * 2 > m->cap) forja_mapa_crecer(m);
    ForjaPar *p = forja_mapa_slot(m, forja_hash_texto(clave), -1, clave);
    if (!p->usado) {
        p->usado = 1;
        p->clave = -1;
        p->clave_texto = clave;
        m->len++;
    }
    p->valor = valor;
    p->valor_texto = es_texto;
}

int64_t forja_mapa_obtener_texto(void *pm, const char *clave) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m || m->len == 0) return 0;
    ForjaPar *p = forja_mapa_slot(m, forja_hash_texto(clave), -1, clave);
    return p->usado ? p->valor : 0;
}

/* `m[clave]` como texto: si la casilla guarda un Texto se devuelve tal cual, si
 * guarda un número se formatea. Es lo que imprime la VM. */
char *forja_mapa_valor_a_texto_i64(void *pm, int64_t clave) {
    ForjaMapa *m = (ForjaMapa *)pm;
    ForjaPar *p = m && m->len ? forja_mapa_slot(m, forja_hash_i64(clave), clave, NULL) : NULL;
    if (!p || !p->usado) return forja_dup("nulo");
    if (p->valor_texto) return forja_dup((const char *)(size_t)p->valor);
    return forja_a_texto(p->valor);
}

char *forja_mapa_valor_a_texto_texto(void *pm, const char *clave) {
    ForjaMapa *m = (ForjaMapa *)pm;
    ForjaPar *p = m && m->len ? forja_mapa_slot(m, forja_hash_texto(clave), -1, clave) : NULL;
    if (!p || !p->usado) return forja_dup("nulo");
    if (p->valor_texto) return forja_dup((const char *)(size_t)p->valor);
    return forja_a_texto(p->valor);
}

/* ── canales e hilos ──────────────────────────────────────────────────── */
/* Canal MPSC simétrico: una cola FIFO con mutex + condición. El emisor y el
 * receptor son el mismo puntero (lo que simplifica `variable e, r = canal()`). */
#ifdef _WIN32
typedef CRITICAL_SECTION ForjaMutex;
typedef CONDITION_VARIABLE ForjaCond;
static void forja_mutex_init(ForjaMutex *m) { InitializeCriticalSection(m); }
static void forja_mutex_lock(ForjaMutex *m) { EnterCriticalSection(m); }
static void forja_mutex_unlock(ForjaMutex *m) { LeaveCriticalSection(m); }
static void forja_cond_init(ForjaCond *c) { InitializeConditionVariable(c); }
static void forja_cond_wait(ForjaCond *c, ForjaMutex *m) { SleepConditionVariableCS(c, m, INFINITE); }
static void forja_cond_signal(ForjaCond *c) { WakeConditionVariable(c); }
#else
#include <pthread.h>
typedef pthread_mutex_t ForjaMutex;
typedef pthread_cond_t ForjaCond;
static void forja_mutex_init(ForjaMutex *m) { pthread_mutex_init(m, NULL); }
static void forja_mutex_lock(ForjaMutex *m) { pthread_mutex_lock(m); }
static void forja_mutex_unlock(ForjaMutex *m) { pthread_mutex_unlock(m); }
static void forja_cond_init(ForjaCond *c) { pthread_cond_init(c, NULL); }
static void forja_cond_wait(ForjaCond *c, ForjaMutex *m) { pthread_cond_wait(c, m); }
static void forja_cond_signal(ForjaCond *c) { pthread_cond_signal(c); }
#endif

typedef struct ForjaNodo {
    int64_t valor;
    struct ForjaNodo *sig;
} ForjaNodo;

typedef struct {
    ForjaMutex mutex;
    ForjaCond cond;
    ForjaNodo *cabeza;
    ForjaNodo *cola;
} ForjaCanal;

void *forja_canal_nuevo(void) {
    ForjaCanal *c = (ForjaCanal *)forja_alloc((int64_t)sizeof(ForjaCanal));
    forja_mutex_init(&c->mutex);
    forja_cond_init(&c->cond);
    c->cabeza = NULL;
    c->cola = NULL;
    return c;
}

void forja_canal_enviar(void *pc, int64_t valor) {
    ForjaCanal *c = (ForjaCanal *)pc;
    if (!c) return;
    ForjaNodo *n = (ForjaNodo *)forja_alloc((int64_t)sizeof(ForjaNodo));
    n->valor = valor;
    n->sig = NULL;
    forja_mutex_lock(&c->mutex);
    if (c->cola) {
        c->cola->sig = n;
    } else {
        c->cabeza = n;
    }
    c->cola = n;
    forja_cond_signal(&c->cond);
    forja_mutex_unlock(&c->mutex);
}

int64_t forja_canal_recibir(void *pc) {
    ForjaCanal *c = (ForjaCanal *)pc;
    if (!c) return 0;
    forja_mutex_lock(&c->mutex);
    while (!c->cabeza) {
        forja_cond_wait(&c->cond, &c->mutex);
    }
    ForjaNodo *n = c->cabeza;
    c->cabeza = n->sig;
    if (!c->cabeza) c->cola = NULL;
    forja_mutex_unlock(&c->mutex);
    return n->valor;
}

/* Intenta recibir sin bloquear: 0 si no hay nada (la VM devuelve nulo). */
int64_t forja_canal_intentar(void *pc) {
    ForjaCanal *c = (ForjaCanal *)pc;
    if (!c) return 0;
    int64_t valor = 0;
    forja_mutex_lock(&c->mutex);
    ForjaNodo *n = c->cabeza;
    if (n) {
        c->cabeza = n->sig;
        if (!c->cabeza) c->cola = NULL;
        valor = n->valor;
    }
    forja_mutex_unlock(&c->mutex);
    return valor;
}

/* Lanza `fn(entorno, indice)` en un hilo del sistema. */
typedef void (*ForjaHiloFn)(void *entorno, int64_t indice);

#ifdef _WIN32
static DWORD WINAPI forja_hilo_trampoline(LPVOID p) {
    void **args = (void **)p;
    ForjaHiloFn fn = (ForjaHiloFn)args[0];
    fn(args[1], 0);
    return 0;
}
void forja_hilo_lanzar(void *fn, void *entorno) {
    void **args = (void **)forja_alloc((int64_t)(2 * sizeof(void *)));
    args[0] = fn;
    args[1] = entorno;
    HANDLE h = CreateThread(NULL, 0, forja_hilo_trampoline, args, 0, NULL);
    if (h) CloseHandle(h);
}
#else
typedef void (*ForjaHiloFnPosix)(void *, int64_t);
static void *forja_hilo_trampoline_posix(void *p) {
    void **args = (void **)p;
    ForjaHiloFnPosix fn = (ForjaHiloFnPosix)args[0];
    fn(args[1], 0);
    return NULL;
}
void forja_hilo_lanzar(void *fn, void *entorno) {
    void **args = (void **)forja_alloc((int64_t)(2 * sizeof(void *)));
    args[0] = fn;
    args[1] = entorno;
    pthread_t t;
    if (pthread_create(&t, NULL, forja_hilo_trampoline_posix, args) == 0) {
        pthread_detach(t);
    }
}
#endif

/* Imprime un mapa como `{clave: valor, …}`. */
void forja_imprimir_mapa(void *pm) {
    ForjaMapa *m = (ForjaMapa *)pm;
    fputc('{', stdout);
    int64_t impresos = 0;
    if (m) {
        for (int64_t i = 0; i < m->cap; i++) {
            ForjaPar *p = &m->tabla[i];
            if (!p->usado) continue;
            if (impresos++) fputs(", ", stdout);
            if (p->clave_texto) {
                fputs(p->clave_texto, stdout);
            } else {
                printf("%lld", (long long)p->clave);
            }
            fputs(": ", stdout);
            printf("%lld", (long long)p->valor);
        }
    }
    fputc('}', stdout);
}

/* ── Exacto (decimal de precisión arbitraria) ─────────────────────────── */
/* Mismo modelo que la VM: coeficiente entero de 128 bits y escala decimal.
 * valor = coef / 10^escala. Las operaciones replican la semántica de
 * `compiler/forja-runtime/src/tipos.rs` (homogeneizar, mul con suma de escalas,
 * div con 20 dígitos extra) para que un programa dé lo mismo en la VM y nativo. */
typedef struct {
    __int128 coef;
    uint32_t escala;
} ForjaExacto;

static void forja_error(const char *msg) {
    fprintf(stderr, "forja: %s\n", msg);
    exit(1);
}

typedef struct {
    __int128 coef;
    uint32_t escala;
} ParExacto;

/* El coeficiente viaja como dos i64: en la ABI de Windows un `__int128` como
 * parámetro se pasa por referencia y el IR no siempre coincide. */
void *forja_exacto_nuevo(int64_t alto, int64_t bajo, int64_t escala) {
    ForjaExacto *e = (ForjaExacto *)forja_alloc((int64_t)sizeof(ForjaExacto));
    unsigned __int128 u = ((unsigned __int128)(uint64_t)alto << 64) | (uint64_t)bajo;
    e->coef = (__int128)u;
    e->escala = (uint32_t)escala;
    return e;
}

static void homogeneizar2(__int128 *a, uint32_t *sa, __int128 *b, uint32_t *sb) {
    if (*sa == *sb) return;
    if (*sa < *sb) {
        __int128 f = 1;
        for (uint32_t i = 0; i < *sb - *sa; i++) {
            if (f > 0 && f > (__int128)0x7fffffffffffffffLL / 10) { /* se deja envolver como la VM */
            }
            f *= 10;
        }
        *a *= f;
        *sa = *sb;
    } else {
        __int128 f = 1;
        for (uint32_t i = 0; i < *sa - *sb; i++) f *= 10;
        *b *= f;
        *sb = *sa;
    }
}

static __int128 pow10_exacto(uint32_t n) {
    __int128 f = 1;
    for (uint32_t i = 0; i < n; i++) f *= 10;
    return f;
}

void *forja_exacto_desde_entero(int64_t v) {
    return forja_exacto_nuevo(v < 0 ? -1 : 0, v, 0);
}

void *forja_exacto_sumar(void *pa, void *pb) {
    ForjaExacto a = *(ForjaExacto *)pa;
    ForjaExacto b = *(ForjaExacto *)pb;
    homogeneizar2(&a.coef, &a.escala, &b.coef, &b.escala);
    __int128 r = a.coef + b.coef;
    return forja_exacto_nuevo((int64_t)(r >> 64), (int64_t)r, a.escala);
}

void *forja_exacto_restar(void *pa, void *pb) {
    ForjaExacto a = *(ForjaExacto *)pa;
    ForjaExacto b = *(ForjaExacto *)pb;
    homogeneizar2(&a.coef, &a.escala, &b.coef, &b.escala);
    __int128 r = a.coef - b.coef;
    return forja_exacto_nuevo((int64_t)(r >> 64), (int64_t)r, a.escala);
}

void *forja_exacto_multiplicar(void *pa, void *pb) {
    ForjaExacto a = *(ForjaExacto *)pa;
    ForjaExacto b = *(ForjaExacto *)pb;
    __int128 r = a.coef * b.coef;
    return forja_exacto_nuevo((int64_t)(r >> 64), (int64_t)r, (int64_t)a.escala + b.escala);
}

void *forja_exacto_dividir(void *pa, void *pb) {
    ForjaExacto a = *(ForjaExacto *)pa;
    ForjaExacto b = *(ForjaExacto *)pb;
    if (b.coef == 0) return NULL; /* la VM devuelve nulo */
    homogeneizar2(&a.coef, &a.escala, &b.coef, &b.escala);
    const uint32_t extra = 20;
    __int128 dividendo = a.coef * pow10_exacto(extra);
    __int128 r = dividendo / b.coef;
    return forja_exacto_nuevo((int64_t)(r >> 64), (int64_t)r, extra);
}

int64_t forja_exacto_comparar(void *pa, void *pb) {
    ForjaExacto a = *(ForjaExacto *)pa;
    ForjaExacto b = *(ForjaExacto *)pb;
    homogeneizar2(&a.coef, &a.escala, &b.coef, &b.escala);
    if (a.coef < b.coef) return -1;
    if (a.coef > b.coef) return 1;
    return 0;
}

int64_t forja_exacto_a_entero(void *pe) {
    if (!pe) return 0;
    ForjaExacto e = *(ForjaExacto *)pe;
    return (int64_t)(e.coef / pow10_exacto(e.escala));
}

double forja_exacto_a_decimal(void *pe) {
    if (!pe) return 0.0;
    ForjaExacto e = *(ForjaExacto *)pe;
    return (double)e.coef / (double)pow10_exacto(e.escala);
}

/* Formato idéntico a `mostrar_exacto` de la VM: punto decimal, sin recortar
 * ceros (150 con escala 2 → "1.50"). */
char *forja_exacto_a_texto(void *pe) {
    if (!pe) return forja_dup("nulo");
    ForjaExacto e = *(ForjaExacto *)pe;
    char crudo[64];
    int n = 0;
    __int128 v = e.coef;
    int negativo = v < 0;
    /* se toma el valor absoluto sin desbordar con el mínimo */
    unsigned __int128 abs_v = negativo ? (unsigned __int128)(-(v + 1)) + 1 : (unsigned __int128)v;
    char digitos[64];
    do {
        digitos[n++] = (char)('0' + (int)(abs_v % 10));
        abs_v /= 10;
    } while (abs_v > 0);
    int total = n + (negativo ? 1 : 0);
    if (e.escala == 0) {
        int k = 0;
        if (negativo) crudo[k++] = '-';
        for (int i = n - 1; i >= 0; i--) crudo[k++] = digitos[i];
        crudo[k] = '\0';
        return forja_dup(crudo);
    }
    /* insertar el punto: (digitos - escala) dígitos enteros */
    int escala = (int)e.escala;
    int k = 0;
    if (negativo) crudo[k++] = '-';
    int enteros = n - escala;
    if (enteros <= 0) {
        crudo[k++] = '0';
        crudo[k++] = '.';
        for (int i = 0; i < -enteros; i++) crudo[k++] = '0';
        for (int i = n - 1; i >= 0; i--) crudo[k++] = digitos[i];
    } else {
        for (int i = n - 1; i >= escala; i--) crudo[k++] = digitos[i];
        crudo[k++] = '.';
        for (int i = escala - 1; i >= 0; i--) crudo[k++] = digitos[i];
    }
    crudo[k] = '\0';
    (void)total;
    return forja_dup(crudo);
}

/* Divide `s` por `sep` y devuelve un arreglo de textos (no incluye el separador). */
void *forja_dividir(const char *s, const char *sep) {
    ForjaArreglo *a = (ForjaArreglo *)forja_arreglo_nuevo(0, 1);
    if (!s || !sep || !*sep) {
        forja_empujar(a, (int64_t)(size_t)forja_dup(s ? s : ""));
        return a;
    }
    size_t lsep = strlen(sep);
    const char *ini = s;
    for (;;) {
        const char *p = strstr(ini, sep);
        size_t n = p ? (size_t)(p - ini) : strlen(ini);
        char *trozo = (char *)forja_alloc((int64_t)n + 1);
        memcpy(trozo, ini, n);
        trozo[n] = '\0';
        forja_empujar(a, (int64_t)(size_t)trozo);
        if (!p) break;
        ini = p + lsep;
    }
    return a;
}

/* Recorta espacios (y \t\r\n) de ambos extremos. */
char *forja_recortar(const char *s) {
    if (!s) return forja_dup("");
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    size_t n = strlen(s);
    while (n > 0) {
        char c = s[n - 1];
        if (c != ' ' && c != '\t' && c != '\n' && c != '\r') break;
        n--;
    }
    char *r = (char *)forja_alloc((int64_t)n + 1);
    memcpy(r, s, n);
    r[n] = '\0';
    return r;
}

/* `esperar(ms)`: duerme el hilo actual. */
void forja_esperar(int64_t ms) {
#ifdef _WIN32
    Sleep((DWORD)(ms > 0 ? ms : 0));
#else
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000;
    nanosleep(&ts, NULL);
#endif
}
