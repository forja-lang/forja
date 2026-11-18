// Runtime de soporte del backend LLVM de Forja.
//
// Se compila junto al `.ll` generado (clang -O3 main.ll forja_rt.c). Sólo expone
// primitivas sin estado global: texto (NUL-terminated, `char*`), arreglos y
// tiempo. Todo lo que asigna memoria con `forja_alloc` vive hasta el final del
// proceso (no hay GC en la ruta nativa todavía).
//
// Modelo de memoria (una sola regla): **nada se libera**.
//   - `forja_alloc` reparte desde bloques de 1 MB (arena); los pedidos grandes
//     van directo a `calloc`. Los bloques se encadenan en `arena_bloques` para
//     poder decidir si un i64 apunta a memoria del programa sin desreferenciarlo
//     (`forja_arreglo_validar`).
//   - los textos nuevos y los arreglos se registran en `registro` (hash
//     `ptr -> tag` con linear probing, crece por duplicado). Ese registro es la
//     única forma de distinguir un objeto de un escalar: los dos viajan en un i64.
//   - `registro` usa `calloc`/`malloc`, no la arena: es infraestructura del
//     runtime, no memoria del programa Forja.
//   - los mapas (`forja_mapa_*`) son lineales con linear probing sobre la arena.
//   - `FORJA_DEBUG_TAGS=1` aborta si al registro entra algo que no sea texto o
//     arreglo (ver `forja_registrar`).
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <ctype.h>
#include <string.h>
#include <time.h>
#include <math.h>
#include <sys/stat.h>

#ifdef _WIN32
#include <windows.h>
#include <direct.h>
#define getcwd _getcwd
#else
#include <unistd.h>
#endif

/* ── memoria ──────────────────────────────────────────────────────────── */

/* Arena: la memoria de un programa Forja no se libera durante la corrida (la
 * ruta nativa no tiene GC), así que en vez de un `calloc` por objeto se reparte
 * desde bloques de 1 MB, alineado a 16 bytes (ABI). Los pedidos grandes van
 * directo a `calloc`. */
#define FORJA_ARENA_BLOQUE (1024 * 1024)
static char *arena_actual = NULL;
static int64_t arena_restante = 0;

/* Bloques repartidos por la arena: sirven para saber si un i64 es un puntero a
 * memoria del programa sin desreferenciarlo (`forja_en_arena`). `es_arena`
 * distingue los bloques de 1 MB (textos y arreglos chicos) de los pedidos
 * grandes, que van directos a `calloc`. */
typedef struct ForjaBloque {
    char *inicio;
    char *fin;
    int es_arena;
    struct ForjaBloque *sig;
} ForjaBloque;
static ForjaBloque *arena_bloques = NULL;
/* Rango de los bloques de arena, y último bloque acertado: los textos se
 * reparten en orden, así que el bloque anterior casi siempre sirve. */
static uintptr_t arena_min = 0;
static uintptr_t arena_max = 0;
static const ForjaBloque *arena_ultimo = NULL;

static void registrar_bloque(char *inicio, int64_t bytes, int es_arena) {
    ForjaBloque *b = (ForjaBloque *)malloc(sizeof(ForjaBloque));
    if (!b) {
        fputs("forja: sin memoria\n", stderr);
        exit(1);
    }
    b->inicio = inicio;
    b->fin = inicio + bytes;
    b->es_arena = es_arena;
    b->sig = arena_bloques;
    arena_bloques = b;
    if (es_arena) {
        uintptr_t ini = (uintptr_t)inicio, fin = (uintptr_t)(inicio + bytes);
        if (!arena_min || ini < arena_min) arena_min = ini;
        if (fin > arena_max) arena_max = fin;
    }
}

/* ¿Es memoria de la arena? Es el test que distingue un texto de un escalar: los
 * textos chicos no se registran uno por uno (eran 12 M de altas en `parseo`). */
static int forja_en_arena(const void *p) {
    uintptr_t x = (uintptr_t)p;
    if (!arena_min || x < arena_min || x >= arena_max) return 0;
    if (arena_ultimo && x >= (uintptr_t)arena_ultimo->inicio &&
        x < (uintptr_t)arena_ultimo->fin) {
        return 1;
    }
    for (const ForjaBloque *b = arena_bloques; b; b = b->sig) {
        if (!b->es_arena) continue;
        if (x >= (uintptr_t)b->inicio && x < (uintptr_t)b->fin) {
            arena_ultimo = b;
            return 1;
        }
    }
    return 0;
}

void *forja_alloc(int64_t bytes) {
    int64_t n = bytes > 0 ? bytes : 1;
    n = (n + 15) & ~(int64_t)15;
    if (n > FORJA_ARENA_BLOQUE / 4) {
        void *p = calloc(1, (size_t)n);
        if (!p) {
            fputs("forja: sin memoria\n", stderr);
            exit(1);
        }
        registrar_bloque((char *)p, n, 0);
        return p;
    }
    if (n > arena_restante) {
        arena_actual = (char *)calloc(1, FORJA_ARENA_BLOQUE);
        if (!arena_actual) {
            fputs("forja: sin memoria\n", stderr);
            exit(1);
        }
        registrar_bloque(arena_actual, FORJA_ARENA_BLOQUE, 1);
        arena_restante = FORJA_ARENA_BLOQUE;
    }
    void *p = arena_actual;
    arena_actual += n;
    arena_restante -= n;
    return p;
}

/* ── identidad de los valores (tag por objeto) ─────────────────────────── */
/* La VM decide el tipo por el **valor** del momento (`6[i]` es `nulo`,
 * `longitud(6)` es 0, `escribir(6)` imprime 6), el backend lo fija por el slot.
 * Para que un slot tipado «arreglo»/«texto» que guarda otra cosa se comporte
 * como la VM, cada objeto de heap se registra con su tag (y los literales de
 * texto se registran al arrancar, ver `forja_registrar_texto`). Lo no
 * registrado es un escalar. */
enum { FORJA_T_TEXTO = 1, FORJA_T_ARREGLO = 2, FORJA_T_MAPA = 3, FORJA_T_CANAL = 4 };

/* Magia en la cabecera de un canal: un texto Forja (UTF-8) nunca empieza por
 * 0xFF, asi que no puede colisionar. */
#define FORJA_CANAL_MAGIA 0xFF43414E414C0001ULL

typedef struct {
    void **claves;
    int64_t *tags;
    int64_t cap;
    int64_t n;
} ForjaRegistro;
static ForjaRegistro registro = {NULL, NULL, 0, 0};

static size_t forja_hash_p(const void *p) {
    uint64_t x = (uint64_t)(uintptr_t)p;
    x ^= x >> 33;
    x *= 0xff51afd7ed558ccdULL;
    x ^= x >> 29;
    return (size_t)x;
}

/* Chequeo opt-in (`FORJA_DEBUG_TAGS=1`). El resto del runtime asume que en el
 * registro **solo** hay objetos de heap (textos y arreglos): un i64 que cae en
 * el registro se trata como objeto (`forja_indice_dinamico`,
 * `forja_a_texto_dinamico`, ...). Si alguien empieza a registrar escalares
 * (decimales, booleanos) esas operaciones mienten en silencio, asi que aca se
 * aborta con el motivo. */
static int forja_debug_tags = -1;
static int forja_debug_tags_activo(void) {
    if (forja_debug_tags < 0) {
        const char *v = getenv("FORJA_DEBUG_TAGS");
        forja_debug_tags = (v && *v && strcmp(v, "0") != 0) ? 1 : 0;
    }
    return forja_debug_tags;
}

static void forja_registrar(void *p, int64_t tag) {
    if (!p) return;
    if (forja_debug_tags_activo() && tag != FORJA_T_TEXTO && tag != FORJA_T_ARREGLO) {
        fprintf(stderr,
                "forja: FORJA_DEBUG_TAGS=1: se registro como objeto un valor con tag %lld "
                "(ptr %p). El registro solo admite textos (1) y arreglos (2): los escalares "
                "no se registran.\n",
                (long long)tag, p);
        abort();
    }
    if ((registro.n + 1) * 2 > registro.cap) {
        int64_t cap = registro.cap ? registro.cap * 2 : 256;
        void **claves = (void **)calloc((size_t)cap, sizeof(void *));
        int64_t *tags = (int64_t *)calloc((size_t)cap, sizeof(int64_t));
        if (!claves || !tags) {
            fputs("forja: sin memoria\n", stderr);
            exit(1);
        }
        for (int64_t i = 0; i < registro.cap; i++) {
            if (!registro.claves[i]) continue;
            size_t j = forja_hash_p(registro.claves[i]) & (size_t)(cap - 1);
            while (claves[j]) j = (j + 1) & (size_t)(cap - 1);
            claves[j] = registro.claves[i];
            tags[j] = registro.tags[i];
        }
        free(registro.claves);
        free(registro.tags);
        registro.claves = claves;
        registro.tags = tags;
        registro.cap = cap;
    }
    size_t j = forja_hash_p(p) & (size_t)(registro.cap - 1);
    while (registro.claves[j] && registro.claves[j] != p) {
        j = (j + 1) & (size_t)(registro.cap - 1);
    }
    if (!registro.claves[j]) {
        registro.claves[j] = p;
        registro.n++;
    } else if (registro.tags[j] != tag && forja_debug_tags_activo()) {
        fprintf(stderr,
                "forja: FORJA_DEBUG_TAGS=1: el ptr %p estaba registrado con tag %lld y se "
                "registra como %lld\n",
                p, (long long)registro.tags[j], (long long)tag);
        abort();
    }
    registro.tags[j] = tag;
}

/* Tag de un valor: 0 si es un escalar (entero, decimal, booleano o nulo). */
static int64_t forja_tag_registrado(const void *p) {
    if (!p || !registro.cap) return 0;
    size_t mask = (size_t)(registro.cap - 1);
    for (size_t j = forja_hash_p(p) & mask; registro.claves[j]; j = (j + 1) & mask) {
        if (registro.claves[j] == p) return registro.tags[j];
    }
    return 0;
}

/* Canal por su cabecera (ver `forja_canal_nuevo`). Solo se lee si el
 * puntero esta en la arena: la cabecera es parte de la misma asignacion. */
static int forja_es_canal(const void *p) {
    if (!forja_en_arena(p)) return 0;
    uint64_t magia = 0;
    memcpy(&magia, (const char *)p - 16, sizeof magia);
    return magia == FORJA_CANAL_MAGIA;
}

static int64_t forja_tag_de(const void *p) {
    int64_t tag = forja_tag_registrado(p);
    if (tag) return tag;
    if (forja_es_canal(p)) return FORJA_T_CANAL;
    /* Un texto de la arena no está en el registro (serían millones de altas):
     * se reconoce por rango. Lo no registrado y fuera de la arena es escalar. */
    return forja_en_arena(p) ? FORJA_T_TEXTO : 0;
}

int64_t forja_tag(void *p) { return forja_tag_de(p); }

/* Los literales de texto viven en el segmento de datos; el programa los
 * registra en `forja_init`. */
void forja_registrar_texto(void *p) { forja_registrar(p, FORJA_T_TEXTO); }

/* Texto nuevo en el heap: los de la arena se reconocen por rango (ver
 * `forja_en_arena`) y no se registran; sólo los grandes van al registro, como
 * los literales (`forja_registrar_texto`). */
static char *forja_texto_alloc(int64_t bytes) {
    char *r = (char *)forja_alloc(bytes);
    if (bytes > FORJA_ARENA_BLOQUE / 4) forja_registrar(r, FORJA_T_TEXTO);
    return r;
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
    char *r = forja_texto_alloc((int64_t)n);
    memcpy(r, s, n);
    return r;
}

char *forja_concat(const char *a, const char *b) {
    /* Un `Texto` nulo (p. ej. `leer()` al final de la entrada) concatena como
     * "nulo", igual que la VM: "Hola, " + nulo -> "Hola, nulo". */
    const char *x = a ? a : "nulo";
    const char *y = b ? b : "nulo";
    size_t la = strlen(x), lb = strlen(y);
    char *r = forja_texto_alloc((int64_t)(la + lb + 1));
    memcpy(r, x, la);
    memcpy(r + la, y, lb + 1);
    return r;
}

char *forja_a_texto(int64_t v) {
    char *r = forja_texto_alloc(32);
    snprintf(r, 32, "%lld", (long long)v);
    return r;
}

char *forja_decimal_a_texto(double v) {
    /* Como el `Display` de `f64` de la VM: la representación más corta que
     * vuelve al mismo double y **siempre en notación decimal** (Rust no usa
     * exponente: 2.7e-08 se imprime como 0.0000000273…). */
    if (v != v) return forja_dup("NaN");
    if (v > 1.7976931348623157e308) return forja_dup("inf");
    if (v < -1.7976931348623157e308) return forja_dup("-inf");
    if (v == 0.0) {
        return forja_dup(signbit(v) ? "-0" : "0");
    }
    // Dígitos significativos mínimos que round-tripean.
    char exp_buf[64];
    int p = 1;
    for (; p < 17; p++) {
        snprintf(exp_buf, sizeof exp_buf, "%.*e", p - 1, v);
        if (strtod(exp_buf, NULL) == v) break;
    }
    int exp10 = 0;
    snprintf(exp_buf, sizeof exp_buf, "%.*e", p - 1, v);
    const char *marca = strchr(exp_buf, 'e');
    if (marca) exp10 = (int)strtol(marca + 1, NULL, 10);
    // Posición del último dígito significativo tras la coma.
    int decimales = p - 1 - exp10;
    if (decimales < 0) decimales = 0;
    // Un double puede necesitar ~1080 caracteres en fijo (desnormales).
    int64_t tam = (int64_t)decimales + 32 + (exp10 > 0 ? exp10 : 0);
    char *r = forja_texto_alloc(tam);
    snprintf(r, (size_t)tam, "%.*f", decimales, v);
    return r;
}

char *forja_texto_nulo(void) { return forja_dup("nulo"); }

/* `s[i]`: un texto de un carácter (nulo si el índice se pasa), como la VM. */
void *forja_texto_indice(const char *s, int64_t i) {
    if (!s) return NULL;
    int64_t n = (int64_t)strlen(s);
    if (i < 0 || i >= n) return NULL;
    char *r = forja_texto_alloc(2);
    r[0] = s[i];
    r[1] = ' ';
    return r;
}

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

/* El 3er argumento es la LONGITUD (como la VM), no el índice final:
   `subcadena("hola mundo", 1, 9)` = "ola mundo". */
char *forja_subcadena(const char *s, int64_t ini, int64_t cant) {
    if (!s) return forja_dup("");
    int64_t n = (int64_t)strlen(s);
    if (ini < 0) ini = 0;
    if (ini > n) ini = n;
    if (cant < 0) cant = 0;
    if (ini + cant > n) cant = n - ini;
    int64_t len = cant;
    char *r = forja_texto_alloc(len + 1);
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
void forja_imprimir_decimal(double v) { fputs(forja_decimal_a_texto(v), stdout); }
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
    /* Al final, para no mover el índice de `datos` en el IR. Lo usa
     * `forja_arreglo_validar` para distinguir un arreglo de otra cosa que
     * apunte a la arena (un texto, un objeto). */
    int64_t magia;
} ForjaArreglo;

#define FORJA_ARREGLO_MAGIA 0x466F726A61417272LL /* "ForjaArr" */

void *forja_arreglo_nuevo(int64_t n, int64_t tipo) {
    ForjaArreglo *a = (ForjaArreglo *)forja_alloc((int64_t)sizeof(ForjaArreglo));
    a->len = n;
    a->cap = n > 4 ? n : 4;
    a->tipo = tipo;
    a->magia = FORJA_ARREGLO_MAGIA;
    a->datos = (int64_t *)forja_alloc(a->cap * (int64_t)sizeof(int64_t));
    forja_registrar(a, FORJA_T_ARREGLO);
    return a;
}

/* `a + b` sobre arreglos: concatenación (`[v] + este.elementos`). Antes el
 * lowering lo trataba como suma de enteros —ptrtoint + add— y reventaba al
 * desreferenciar la suma de dos punteros. Las casillas viajan como i64. */
void *forja_arreglo_concat(void *pa, void *pb) {
    ForjaArreglo *a = (ForjaArreglo *)pa;
    ForjaArreglo *b = (ForjaArreglo *)pb;
    if (!a) return b ? pb : forja_arreglo_nuevo(0, 0);
    if (!b) return pa;
    /* El 1 dicen que las casillas son textos (punteros) aunque estén vacíos. */
    int64_t tipo = a->len ? a->tipo : b->tipo;
    ForjaArreglo *r = (ForjaArreglo *)forja_arreglo_nuevo(a->len + b->len, tipo);
    for (int64_t i = 0; i < a->len; i++) r->datos[i] = a->datos[i];
    for (int64_t i = 0; i < b->len; i++) r->datos[a->len + i] = b->datos[i];
    return r;
}

/* Casilla de un arreglo que guarda arreglos (`este.matriz[i]`, `adyacencia[i]`).
 * La VM indexa cualquier cosa y devuelve `nulo`; acá la casilla se lee con el
 * registro de objetos: si no es un arreglo, es `nulo`. Nunca se desreferencia un
 * número (esa era la causa de los segfaults). */
void *forja_arreglo_validar(int64_t casilla) {
    void *p = (void *)(intptr_t)casilla;
    return forja_tag_de(p) == FORJA_T_ARREGLO ? p : NULL;
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

/* ── operaciones por valor (como la VM) ────────────────────────────────── */
/* La VM indexa, mide e imprime cualquier valor según su tipo del momento; acá
 * se pregunta el tag del objeto (ver `forja_tag_de`). Un slot tipado «arreglo»
 * que guarda un entero da longitud 0, índice `nulo` y se imprime como 1. */

int64_t forja_arreglo_obtener(void *pv, int64_t i) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    /* Fuera de rango es `nulo` en la VM, no un error. */
    if (!a || i < 0 || i >= a->len) return 0;
    return a->datos[i];
}

void forja_arreglo_fijar(void *pv, int64_t i, int64_t v) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    if (!a || i < 0) return;
    /* `arr[i] = v` más allá del final crece y deja huecos `nulo`, como la VM. */
    if (i >= a->len) {
        forja_arreglo_crecer(a, i + 1);
        for (int64_t k = a->len; k < i; k++) a->datos[k] = 0;
        a->len = i + 1;
        a->datos[i] = v;
        return;
    }
    a->datos[i] = v;
}

/* `v[i]`: arreglo → su casilla, texto → el carácter, otra cosa → `nulo`. */
int64_t forja_indice_dinamico(void *v, int64_t i) {
    switch (forja_tag_de(v)) {
        case FORJA_T_ARREGLO:
            return forja_arreglo_obtener(v, i);
        case FORJA_T_TEXTO:
            return (int64_t)(intptr_t)forja_texto_indice((const char *)v, i);
        default:
            return 0;
    }
}

/* `longitud(v)`: arreglo → longitud, texto → strlen, otra cosa → 0. */
int64_t forja_longitud_dinamica(void *v) {
    switch (forja_tag_de(v)) {
        case FORJA_T_ARREGLO:
            return forja_arreglo_longitud(v);
        case FORJA_T_TEXTO:
            return forja_longitud((const char *)v);
        default:
            return 0;
    }
}

void forja_imprimir_arreglo(void *pv);
char *forja_arreglo_a_texto(void *pv);

/* `escribir(v)` cuando el tipo del slot es una suposición. */
void forja_imprimir_dinamico(void *v) {
    switch (forja_tag_de(v)) {
        case FORJA_T_CANAL:
            forja_imprimir_texto("[<CanalEmisor>,<CanalReceptor>]");
            return;
        case FORJA_T_ARREGLO:
            forja_imprimir_arreglo(v);
            return;
        case FORJA_T_TEXTO:
            forja_imprimir_texto((const char *)v);
            return;
        default:
            forja_imprimir_entero((int64_t)(intptr_t)v);
    }
}

/* `"" + v` cuando el tipo del slot es una suposición. */
/* `a_codigo(c)`: codigo Unicode del primer caracter de `s` (0 si esta vacio).
 * `a_caracter(n)`: el texto UTF-8 de ese codigo. */
int64_t forja_a_codigo(const char *s) {
    if (!s || !*s) return 0;
    const unsigned char *p = (const unsigned char *)s;
    if (p[0] < 0x80) return p[0];
    if ((p[0] & 0xE0) == 0xC0) return ((p[0] & 0x1F) << 6) | (p[1] & 0x3F);
    if ((p[0] & 0xF0) == 0xE0)
        return ((p[0] & 0x0F) << 12) | ((p[1] & 0x3F) << 6) | (p[2] & 0x3F);
    if ((p[0] & 0xF8) == 0xF0)
        return ((p[0] & 0x07) << 18) | ((p[1] & 0x3F) << 12) | ((p[2] & 0x3F) << 6) |
               (p[3] & 0x3F);
    return 0;
}

char *forja_a_caracter(int64_t codigo) {
    char *r = forja_texto_alloc(8);
    uint32_t c = (uint32_t)codigo;
    if (c < 0x80) {
        r[0] = (char)c;
        r[1] = 0;
    } else if (c < 0x800) {
        r[0] = (char)(0xC0 | (c >> 6));
        r[1] = (char)(0x80 | (c & 0x3F));
        r[2] = 0;
    } else if (c < 0x10000) {
        r[0] = (char)(0xE0 | (c >> 12));
        r[1] = (char)(0x80 | ((c >> 6) & 0x3F));
        r[2] = (char)(0x80 | (c & 0x3F));
        r[3] = 0;
    } else {
        r[0] = (char)(0xF0 | (c >> 18));
        r[1] = (char)(0x80 | ((c >> 12) & 0x3F));
        r[2] = (char)(0x80 | ((c >> 6) & 0x3F));
        r[3] = (char)(0x80 | (c & 0x3F));
        r[4] = 0;
    }
    return r;
}

/* `reemplazar(texto, viejo, nuevo)`: todas las apariciones, como el `replace`
 * de Rust que usa la VM. */
char *forja_reemplazar(const char *s, const char *viejo, const char *nuevo) {
    if (!s) return forja_dup("nulo");
    if (!viejo || !*viejo) return forja_dup(s);
    if (!nuevo) nuevo = "";
    size_t ls = strlen(s), lv = strlen(viejo), ln = strlen(nuevo);
    size_t apariciones = 0;
    const char *p = s;
    while ((p = strstr(p, viejo)) != NULL) {
        apariciones++;
        p += lv;
    }
    if (!apariciones) return forja_dup(s);
    char *r = forja_texto_alloc((int64_t)(ls - apariciones * lv + apariciones * ln + 1));
    char *o = r;
    const char *i = s;
    const char *encontrado;
    while ((encontrado = strstr(i, viejo)) != NULL) {
        size_t tramo = (size_t)(encontrado - i);
        memcpy(o, i, tramo);
        o += tramo;
        memcpy(o, nuevo, ln);
        o += ln;
        i = encontrado + lv;
    }
    strcpy(o, i);
    return r;
}

char *forja_a_texto_dinamico(void *v) {
    switch (forja_tag_de(v)) {
        case FORJA_T_CANAL:
            return forja_dup("[<CanalEmisor>,<CanalReceptor>]");
        case FORJA_T_ARREGLO:
            return forja_arreglo_a_texto(v);
        case FORJA_T_TEXTO:
            return forja_dup((const char *)v);
        default:
            return forja_a_texto((int64_t)(intptr_t)v);
    }
}

/* Imprime un arreglo como `[1,2,3]` / `[a,b]`. Cada casilla se imprime por su
 * tag: un arreglo es heterogéneo y el `tipo` declarado puede no describirlo
 * (`mapa(numeros, f)` que devuelve textos, casillas con `nulo`). */
void forja_imprimir_dinamico(void *v);

void forja_imprimir_arreglo(void *pv) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    fputc('[', stdout);
    if (a) {
        for (int64_t i = 0; i < a->len; i++) {
            if (i) fputc(',', stdout);
            forja_imprimir_dinamico((void *)(intptr_t)a->datos[i]);
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
        void *dato = (void *)(intptr_t)a->datos[i];
        switch (forja_tag_de(dato)) {
            case FORJA_T_ARREGLO:
                n += (int64_t)strlen(forja_arreglo_a_texto(dato));
                break;
            case FORJA_T_TEXTO:
                n += (int64_t)strlen((const char *)dato);
                break;
            default:
                n += 24;
        }
        n += 1; /* separador */
    }
    char *r = forja_texto_alloc(n + 1);
    char *p = r;
    *p++ = '[';
    for (int64_t i = 0; i < a->len; i++) {
        if (i) *p++ = ',';
        void *dato = (void *)(intptr_t)a->datos[i];
        switch (forja_tag_de(dato)) {
            case FORJA_T_ARREGLO:
            case FORJA_T_TEXTO: {
                char *t = forja_tag_de(dato) == FORJA_T_ARREGLO
                              ? forja_arreglo_a_texto(dato)
                              : (char *)dato;
                size_t l = strlen(t);
                memcpy(p, t, l);
                p += l;
                break;
            }
            default:
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
    forja_registrar(m, FORJA_T_MAPA);
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

/* Clave de mapa decidida por el **valor**, no por el slot: la VM indexa con el
 * valor del momento. `m[partes[i][0]] = v` llega como los bits de un Texto en
 * un slot Entero; guardarlo con `fijar_i64` dejaba la clave como número y la
 * entrada se perdía (`fusionar` de std/colecciones, ejemplo `732`). */
void forja_mapa_fijar_dinamico(void *pm, int64_t clave, int64_t valor, int64_t es_texto) {
    if (forja_tag_de((void *)(intptr_t)clave) == FORJA_T_TEXTO)
        forja_mapa_fijar_texto(pm, (const char *)(intptr_t)clave, valor, es_texto);
    else
        forja_mapa_fijar_i64(pm, clave, valor, es_texto);
}

int64_t forja_mapa_obtener_dinamico(void *pm, int64_t clave) {
    if (forja_tag_de((void *)(intptr_t)clave) == FORJA_T_TEXTO)
        return forja_mapa_obtener_texto(pm, (const char *)(intptr_t)clave);
    return forja_mapa_obtener_i64(pm, clave);
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

/* Los canales llevan una cabecera con una magia que no puede salir de un
 * texto Forja (UTF-8 valido nunca empieza con 0xFF): asi `forja_tag_de`
 * distingue un canal de un texto sin una tabla por instancia. */
void *forja_canal_nuevo(void) {
    char *base = (char *)forja_alloc((int64_t)(16 + sizeof(ForjaCanal)));
    uint64_t magia = FORJA_CANAL_MAGIA;
    memcpy(base, &magia, sizeof magia);
    ForjaCanal *c = (ForjaCanal *)(base + 16);
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

/* Lanza `fn(entorno, indice)` en un hilo del sistema y devuelve un handle para
 * `forja_hilo_unir`. El cuerpo del hilo devuelve sus bits como `int64_t`
 * (`retornar x` dentro de `hilo { … }`). */
typedef int64_t (*ForjaHiloFn)(void *entorno, int64_t indice);

typedef struct {
#ifdef _WIN32
    HANDLE hilo;
#else
    pthread_t hilo;
    int lanzado;
#endif
    ForjaHiloFn fn;
    void *entorno;
    int64_t valor;
} ForjaHilo;

/* Registro de handles vivos: `unir(x)` sobre algo que no es un hilo (la VM no
 * tiene concurrencia real y devuelve el valor tal cual) no debe desreferenciar
 * un puntero cualquiera (`async_await/basico`: `suma(1, 2).unir()`). */
#define FORJA_MAX_HILOS 4096
static ForjaHilo *forja_hilos_reg[FORJA_MAX_HILOS];
static int forja_hilos_n = 0;
static ForjaMutex forja_hilos_mutex;
static int forja_hilos_mutex_listo = 0;

static void forja_hilo_registrar(ForjaHilo *h) {
    if (!forja_hilos_mutex_listo) {
        forja_mutex_init(&forja_hilos_mutex);
        forja_hilos_mutex_listo = 1;
    }
    forja_mutex_lock(&forja_hilos_mutex);
    if (forja_hilos_n < FORJA_MAX_HILOS) forja_hilos_reg[forja_hilos_n++] = h;
    forja_mutex_unlock(&forja_hilos_mutex);
}

static ForjaHilo *forja_hilo_buscar(void *hilo) {
    ForjaHilo *encontrado = NULL;
    if (!forja_hilos_mutex_listo) return NULL;
    forja_mutex_lock(&forja_hilos_mutex);
    for (int i = 0; i < forja_hilos_n; i++) {
        if ((void *)forja_hilos_reg[i] == hilo) {
            encontrado = forja_hilos_reg[i];
            break;
        }
    }
    forja_mutex_unlock(&forja_hilos_mutex);
    return encontrado;
}

#ifdef _WIN32
static DWORD WINAPI forja_hilo_trampoline(LPVOID p) {
    ForjaHilo *h = (ForjaHilo *)p;
    h->valor = h->fn(h->entorno, 0);
    return 0;
}
void *forja_hilo_lanzar(void *fn, void *entorno) {
    /* Fuera de la arena: un handle no es un texto ni un arreglo, y el registro
     * de tags lo clasificaria como texto por caer en un bloque de la arena. */
    ForjaHilo *h = (ForjaHilo *)malloc(sizeof(ForjaHilo));
    if (!h) return NULL;
    h->fn = (ForjaHiloFn)fn;
    h->entorno = entorno;
    h->valor = 0;
    h->hilo = CreateThread(NULL, 0, forja_hilo_trampoline, h, 0, NULL);
    forja_hilo_registrar(h);
    return h;
}
int64_t forja_hilo_unir(void *hilo) {
    ForjaHilo *h = forja_hilo_buscar(hilo);
    if (!h) return (int64_t)(intptr_t)hilo;
    if (h->hilo) {
        WaitForSingleObject(h->hilo, INFINITE);
        CloseHandle(h->hilo);
        h->hilo = NULL;
    }
    return h->valor;
}
#else
static void *forja_hilo_trampoline_posix(void *p) {
    ForjaHilo *h = (ForjaHilo *)p;
    h->valor = h->fn(h->entorno, 0);
    return NULL;
}
void *forja_hilo_lanzar(void *fn, void *entorno) {
    /* Fuera de la arena: un handle no es un texto ni un arreglo, y el registro
     * de tags lo clasificaria como texto por caer en un bloque de la arena. */
    ForjaHilo *h = (ForjaHilo *)malloc(sizeof(ForjaHilo));
    if (!h) return NULL;
    h->fn = (ForjaHiloFn)fn;
    h->entorno = entorno;
    h->valor = 0;
    h->lanzado = pthread_create(&h->hilo, NULL, forja_hilo_trampoline_posix, h) == 0;
    forja_hilo_registrar(h);
    return h;
}
int64_t forja_hilo_unir(void *hilo) {
    ForjaHilo *h = forja_hilo_buscar(hilo);
    if (!h) return (int64_t)(intptr_t)hilo;
    if (h->lanzado) {
        pthread_join(h->hilo, NULL);
        h->lanzado = 0;
    }
    return h->valor;
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
        char *trozo = forja_texto_alloc((int64_t)n + 1);
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
    char *r = forja_texto_alloc((int64_t)n + 1);
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

/* ── entrada/salida de línea ──────────────────────────────────────────── */
/* `leer()`: una línea de stdin sin el `\n`; al final de la entrada, `nulo`
 * (NULL), como la VM. */
char *forja_leer_linea(void) {
    size_t cap = 128, n = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) return NULL;
    int c;
    while ((c = fgetc(stdin)) != EOF && c != '\n') {
        if (n + 1 >= cap) {
            cap *= 2;
            char *nuevo = (char *)realloc(buf, cap);
            if (!nuevo) {
                free(buf);
                return NULL;
            }
            buf = nuevo;
        }
        buf[n++] = (char)c;
    }
    if (c == EOF && n == 0) {
        free(buf);
        return NULL;
    }
    if (n > 0 && buf[n - 1] == '\r') n--;
    buf[n] = '\0';
    /* Como la VM (`texto_de_lectura`): recorta los espacios de los extremos.
     * Una linea en blanco es texto vacio; solo el fin de entrada es `nulo`. */
    size_t ini = 0, fin = n;
    while (ini < fin && isspace((unsigned char)buf[ini])) ini++;
    while (fin > ini && isspace((unsigned char)buf[fin - 1])) fin--;
    char *r = forja_texto_alloc((int64_t)(fin - ini) + 1);
    memcpy(r, buf + ini, fin - ini);
    r[fin - ini] = '\0';
    free(buf);
    return r;
}

/* ── hexadecimal ─────────────────────────────────────────────────────── */
static int forja_hex_valor(unsigned char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    return -1;
}

/* `_hex_decodificar_nativo`: bytes hex → texto (UTF-8 best effort, como
 * `String::from_utf8_lossy` de la VM); "" si el hex es inválido. */
char *forja_hex_decodificar(const char *s) {
    if (!s) return forja_dup("");
    size_t n = strlen(s);
    if (n < 2 || n % 2 != 0) return forja_dup("");
    char *r = forja_texto_alloc((int64_t)(n / 2) + 1);
    for (size_t i = 0; i < n / 2; i++) {
        int alto = forja_hex_valor((unsigned char)s[i * 2]);
        int bajo = forja_hex_valor((unsigned char)s[i * 2 + 1]);
        if (alto < 0 || bajo < 0) {
            return forja_dup("");
        }
        r[i] = (char)((alto << 4) | bajo);
    }
    r[n / 2] = '\0';
    return r;
}

/* ── argumentos del proceso ───────────────────────────────────────────── */
/* `main` recibe `argc`/`argv` y los guarda acá; `_sistema_argumentos()` los
 * devuelve como arreglo de textos (el primero es la ruta del ejecutable). */
static int32_t forja_argc = 0;
static char **forja_argv = NULL;

void forja_guardar_args(int32_t argc, void *argv) {
    forja_argc = argc;
    forja_argv = (char **)argv;
}

void *forja_argumentos(void) {
    int64_t n = forja_argc > 0 ? forja_argc : 0;
    ForjaArreglo *a = (ForjaArreglo *)forja_arreglo_nuevo(n, 1);
    for (int64_t i = 0; i < n; i++) {
        const char *arg = forja_argv ? forja_argv[i] : "";
        a->datos[i] = (int64_t)(intptr_t)forja_dup(arg ? arg : "");
    }
    return a;
}

/* ── archivos: copiar ─────────────────────────────────────────────────── */
/* `_archivo_copiar(origen, destino)`: 0 si copió, -1 si no (como la VM). */
int64_t forja_archivo_copiar(const char *origen, const char *destino) {
    if (!origen || !destino || !*origen || !*destino) return -1;
    FILE *in = fopen(origen, "rb");
    if (!in) return -1;
    FILE *out = fopen(destino, "wb");
    if (!out) {
        fclose(in);
        return -1;
    }
    char buf[65536];
    size_t n;
    int64_t rc = 0;
    while ((n = fread(buf, 1, sizeof buf, in)) > 0) {
        if (fwrite(buf, 1, n, out) != n) {
            rc = -1;
            break;
        }
    }
    if (ferror(in)) rc = -1;
    fclose(in);
    if (fclose(out) != 0) rc = -1;
    return rc;
}

/* ── fechas ───────────────────────────────────────────────────────────── */
static const char *const FORJA_DIAS[] = {"domingo", "lunes", "martes", "miercoles",
                                         "jueves", "viernes", "sabado"};
static const char *const FORJA_MESES[] = {"enero",     "febrero", "marzo",     "abril",
                                          "mayo",      "junio",   "julio",     "agosto",
                                          "septiembre", "octubre", "noviembre", "diciembre"};

/* `_fecha_desde_timestamp(ts)`: "año|mes|dia|hora|minuto|segundo|dia_semana|mes_nombre",
 * con el algoritmo civil (el mismo de `forja-native`). */
char *forja_fecha_desde_timestamp(int64_t ts) {
    int64_t dias = ts / 86400;
    if (ts % 86400 < 0) dias -= 1; /* división euclidiana */
    int64_t resto = ts - dias * 86400;
    int64_t hora = resto / 3600, minuto = (resto % 3600) / 60, segundo = resto % 60;

    int64_t z = dias + 719468;
    int64_t era = (z >= 0 ? z : z - 146096) / 146097;
    int64_t doe = z - era * 146097;
    int64_t yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    int64_t y = yoe + era * 400;
    int64_t doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    int64_t mp = (5 * doy + 2) / 153;
    int64_t d = doy - (153 * mp + 2) / 5 + 1;
    int64_t m = mp < 10 ? mp + 3 : mp - 9;
    if (m <= 2) y += 1;

    static const int t[] = {0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4};
    int64_t ya = m < 3 ? y - 1 : y;
    int64_t dow = (ya + ya / 4 - ya / 100 + ya / 400 + t[m - 1] + d) % 7;

    char *r = forja_texto_alloc(160);
    snprintf(r, 160, "%lld|%lld|%lld|%lld|%lld|%lld|%s|%s", (long long)y, (long long)m,
             (long long)d, (long long)hora, (long long)minuto, (long long)segundo,
             FORJA_DIAS[dow], FORJA_MESES[m - 1]);
    return r;
}

/* ── sha256 (FIPS 180-4) ──────────────────────────────────────────────── */
static uint32_t forja_rotr(uint32_t x, int n) { return (x >> n) | (x << (32 - n)); }

static void forja_sha256_bloque(uint32_t h[8], const unsigned char *p) {
    static const uint32_t K[64] = {
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2};
    uint32_t w[64];
    for (int i = 0; i < 16; i++) {
        w[i] = ((uint32_t)p[i * 4] << 24) | ((uint32_t)p[i * 4 + 1] << 16) |
               ((uint32_t)p[i * 4 + 2] << 8) | (uint32_t)p[i * 4 + 3];
    }
    for (int i = 16; i < 64; i++) {
        uint32_t s0 = forja_rotr(w[i - 15], 7) ^ forja_rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
        uint32_t s1 = forja_rotr(w[i - 2], 17) ^ forja_rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16] + s0 + w[i - 7] + s1;
    }
    uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
    for (int i = 0; i < 64; i++) {
        uint32_t S1 = forja_rotr(e, 6) ^ forja_rotr(e, 11) ^ forja_rotr(e, 25);
        uint32_t ch = (e & f) ^ (~e & g);
        uint32_t t1 = hh + S1 + ch + K[i] + w[i];
        uint32_t S0 = forja_rotr(a, 2) ^ forja_rotr(a, 13) ^ forja_rotr(a, 22);
        uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
        uint32_t t2 = S0 + maj;
        hh = g;
        g = f;
        f = e;
        e = d + t1;
        d = c;
        c = b;
        b = a;
        a = t1 + t2;
    }
    h[0] += a;
    h[1] += b;
    h[2] += c;
    h[3] += d;
    h[4] += e;
    h[5] += f;
    h[6] += g;
    h[7] += hh;
}

/* `_sha256(texto)`: el hash en hexadecimal minúsculas (64 caracteres). */
char *forja_sha256(const char *s) {
    const unsigned char *datos = (const unsigned char *)(s ? s : "");
    size_t n = s ? strlen(s) : 0;
    uint32_t h[8] = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                     0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    size_t bloques = n / 64;
    for (size_t i = 0; i < bloques; i++) forja_sha256_bloque(h, datos + i * 64);
    /* Relleno: 0x80, ceros y la longitud en bits (big endian), en 1 o 2 bloques. */
    unsigned char resto[128];
    size_t sobra = n - bloques * 64;
    memcpy(resto, datos + bloques * 64, sobra);
    resto[sobra] = 0x80;
    size_t total = sobra + 1 + 8 <= 64 ? 64 : 128;
    memset(resto + sobra + 1, 0, total - sobra - 1);
    uint64_t bits = (uint64_t)n * 8;
    for (int i = 0; i < 8; i++) resto[total - 1 - i] = (unsigned char)(bits >> (8 * i));
    forja_sha256_bloque(h, resto);
    if (total == 128) forja_sha256_bloque(h, resto + 64);

    char *r = forja_texto_alloc(65);
    for (int i = 0; i < 8; i++) sprintf(r + i * 8, "%08x", h[i]);
    r[64] = ' ';
    return r;
}

/* ── base64 (RFC 4648, igual que `forja-native/src/base64.rs`) ────────── */
static const char FORJA_B64[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

char *forja_base64_codificar(const char *s) {
    if (!s) s = "";
    size_t n = strlen(s);
    size_t bloques = n / 3, resto = n % 3;
    size_t cap = bloques * 4 + (resto ? 4 : 0);
    char *r = forja_texto_alloc((int64_t)cap + 1);
    char *o = r;
    for (size_t i = 0; i < bloques; i++) {
        unsigned b0 = (unsigned char)s[i * 3];
        unsigned b1 = (unsigned char)s[i * 3 + 1];
        unsigned b2 = (unsigned char)s[i * 3 + 2];
        unsigned v = (b0 << 16) | (b1 << 8) | b2;
        *o++ = FORJA_B64[(v >> 18) & 0x3F];
        *o++ = FORJA_B64[(v >> 12) & 0x3F];
        *o++ = FORJA_B64[(v >> 6) & 0x3F];
        *o++ = FORJA_B64[v & 0x3F];
    }
    if (resto == 1) {
        unsigned v = ((unsigned char)s[n - 1]) << 16;
        *o++ = FORJA_B64[(v >> 18) & 0x3F];
        *o++ = FORJA_B64[(v >> 12) & 0x3F];
        *o++ = '=';
        *o++ = '=';
    } else if (resto == 2) {
        unsigned v = (((unsigned char)s[n - 2]) << 16) | (((unsigned char)s[n - 1]) << 8);
        *o++ = FORJA_B64[(v >> 18) & 0x3F];
        *o++ = FORJA_B64[(v >> 12) & 0x3F];
        *o++ = FORJA_B64[(v >> 6) & 0x3F];
        *o++ = '=';
    }
    *o = '\0';
    return r;
}

static int forja_b64_valor(unsigned char c) {
    if (c == '=') return 0;
    const char *p = strchr(FORJA_B64, (char)c);
    return p ? (int)(p - FORJA_B64) : -1;
}

/* Devuelve "" si el texto no es base64 valido (la capa Forja arma el `Error`). */
char *forja_base64_decodificar(const char *s) {
    if (!s || !*s) return forja_dup("");
    size_t n = strlen(s);
    if (n % 4 != 0) return forja_dup("");
    size_t relleno = 0;
    if (s[n - 1] == '=') relleno = (s[n - 2] == '=') ? 2 : 1;
    size_t bloques = n / 4;
    char *r = forja_texto_alloc((int64_t)(bloques * 3) + 1);
    size_t o = 0;
    for (size_t i = 0; i < bloques; i++) {
        int c0 = forja_b64_valor((unsigned char)s[i * 4]);
        int c1 = forja_b64_valor((unsigned char)s[i * 4 + 1]);
        int c2 = forja_b64_valor((unsigned char)s[i * 4 + 2]);
        int c3 = forja_b64_valor((unsigned char)s[i * 4 + 3]);
        if (c0 < 0 || c1 < 0 || c2 < 0 || c3 < 0) {
            return forja_dup("");
        }
        unsigned v = ((unsigned)c0 << 18) | ((unsigned)c1 << 12) | ((unsigned)c2 << 6) |
                     (unsigned)c3;
        r[o++] = (char)(v >> 16);
        if (relleno < 2 || i + 1 < bloques) r[o++] = (char)(v >> 8);
        if (relleno == 0 || i + 1 < bloques) r[o++] = (char)v;
    }
    r[o] = '\0';
    return r;
}

/* `escribir_linea(s)`: como `escribir` pero con salto de línea. */
void forja_escribir_linea(const char *s) {
    fputs(s ? s : "nulo", stdout);
    fputc('\n', stdout);
}

/* `unir_elementos(arreglo, separador)`: une las casillas como texto, por tag. */
char *forja_unir_elementos(void *pv, const char *sep) {
    ForjaArreglo *a = (ForjaArreglo *)pv;
    const char *s = sep ? sep : "";
    if (!a) return forja_dup("nulo");
    int64_t n = 1;
    for (int64_t i = 0; i < a->len; i++) {
        void *dato = (void *)(intptr_t)a->datos[i];
        switch (forja_tag_de(dato)) {
            case FORJA_T_ARREGLO:
            case FORJA_T_TEXTO:
                n += (int64_t)strlen(forja_tag_de(dato) == FORJA_T_ARREGLO
                                         ? forja_arreglo_a_texto(dato)
                                         : (const char *)dato);
                break;
            default:
                n += 24;
        }
        if (i) n += (int64_t)strlen(s);
    }
    char *r = forja_texto_alloc(n + 1);
    char *p = r;
    for (int64_t i = 0; i < a->len; i++) {
        if (i) {
            size_t l = strlen(s);
            memcpy(p, s, l);
            p += l;
        }
        void *dato = (void *)(intptr_t)a->datos[i];
        int64_t tag = forja_tag_de(dato);
        if (tag == FORJA_T_ARREGLO) {
            const char *t = forja_arreglo_a_texto(dato);
            size_t l = strlen(t);
            memcpy(p, t, l);
            p += l;
        } else if (tag == FORJA_T_TEXTO) {
            const char *t = (const char *)dato;
            size_t l = strlen(t);
            memcpy(p, t, l);
            p += l;
        } else {
            p += sprintf(p, "%lld", (long long)a->datos[i]);
        }
    }
    *p = '\0';
    return r;
}

/* `tipo(v)` cuando el tipo estático no alcanza: los objetos se distinguen por su
 * tag; un escalar sin tag es un entero. */
char *forja_tipo_de(void *v) {
    if (!v) return forja_dup("nulo");
    switch (forja_tag_de(v)) {
        case FORJA_T_TEXTO:
            return forja_dup("texto");
        case FORJA_T_ARREGLO:
            return forja_dup("arreglo");
        case FORJA_T_MAPA:
            return forja_dup("mapa");
        case FORJA_T_CANAL:
            /* La VM modela el canal como `[emisor, receptor]`: `tipo()` dice
             * "objeto". */
            return forja_dup("objeto");
        default:
            return forja_dup("entero");
    }
}

/* ── introspección de mapas (claves/valores/pares) ────────────────────── */
/* Claves en orden estable: primero las de texto (alfabético) y después las
 * enteras. `_mapa_valores` sigue el mismo orden y `_mapa_pares` devuelve
 * `[clave, valor]`. */
/* Índices de las casillas usadas, con las claves de texto primero. */
static int64_t mapa_indices(ForjaMapa *m, int64_t **out) {
    int64_t *idx = (int64_t *)forja_alloc(m->len > 0 ? m->len * 8 : 8);
    int64_t n = 0;
    for (int64_t i = 0; i < m->cap; i++) {
        if (m->tabla[i].usado) idx[n++] = i;
    }
    /* selección por clave: texto antes que enteras, cada grupo ordenado */
    for (int64_t i = 0; i < n; i++) {
        for (int64_t j = i + 1; j < n; j++) {
            ForjaPar *a = &m->tabla[idx[i]], *b = &m->tabla[idx[j]];
            int texto_a = a->clave_texto != NULL, texto_b = b->clave_texto != NULL;
            int intercambiar = 0;
            if (!texto_a && texto_b) {
                intercambiar = 1;
            } else if (texto_a && texto_b) {
                intercambiar = strcmp(a->clave_texto, b->clave_texto) > 0;
            } else if (!texto_a && !texto_b) {
                intercambiar = a->clave > b->clave;
            }
            if (intercambiar) {
                int64_t t = idx[i];
                idx[i] = idx[j];
                idx[j] = t;
            }
        }
    }
    *out = idx;
    return n;
}

void *forja_mapa_claves(void *pm) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m) return forja_arreglo_nuevo(0, 1);
    int64_t *idx;
    int64_t n = mapa_indices(m, &idx);
    ForjaArreglo *r = (ForjaArreglo *)forja_arreglo_nuevo(n, 1);
    for (int64_t i = 0; i < n; i++) {
        ForjaPar *p = &m->tabla[idx[i]];
        if (p->clave_texto) {
            r->datos[i] = (int64_t)(intptr_t)forja_dup(p->clave_texto);
        } else {
            char *t = forja_a_texto(p->clave);
            r->datos[i] = (int64_t)(intptr_t)t;
        }
    }
    return r;
}

void *forja_mapa_valores(void *pm) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m) return forja_arreglo_nuevo(0, 0);
    int64_t *idx;
    int64_t n = mapa_indices(m, &idx);
    ForjaArreglo *r = (ForjaArreglo *)forja_arreglo_nuevo(n, 0);
    for (int64_t i = 0; i < n; i++) {
        r->datos[i] = m->tabla[idx[i]].valor;
    }
    return r;
}

void *forja_mapa_pares(void *pm) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m) return forja_arreglo_nuevo(0, 0);
    int64_t *idx;
    int64_t n = mapa_indices(m, &idx);
    ForjaArreglo *r = (ForjaArreglo *)forja_arreglo_nuevo(n, 0);
    for (int64_t i = 0; i < n; i++) {
        ForjaPar *p = &m->tabla[idx[i]];
        ForjaArreglo *fila = (ForjaArreglo *)forja_arreglo_nuevo(2, 0);
        if (p->clave_texto) {
            fila->datos[0] = (int64_t)(intptr_t)forja_dup(p->clave_texto);
        } else {
            fila->datos[0] = p->clave;
        }
        fila->datos[1] = p->valor;
        r->datos[i] = (int64_t)(intptr_t)fila;
    }
    return r;
}

/* ── primitivas de la stdlib (mismas que `forja-native`) ──────────────── */
/* Los módulos `std/archivo`, `std/env` y `std/sistema` llaman a estas
 * primitivas y esperan el **centinela -1** en vez de un aborto: un archivo
 * inexistente es un `Error(...)` manejable, como en la VM. */

/* `_archivo_leer(ruta)`: contenido, o -1. Devuelve el texto como i64 (los bits
 * del puntero) porque el resultado puede ser `texto` o el entero -1. */
int64_t forja_archivo_leer(const char *ruta) {
    if (!ruta || !*ruta) return -1;
    FILE *f = fopen(ruta, "rb");
    if (!f) return -1;
    if (fseek(f, 0, SEEK_END) != 0) {
        fclose(f);
        return -1;
    }
    long n = ftell(f);
    if (n < 0) {
        fclose(f);
        return -1;
    }
    rewind(f);
    char *buf = forja_texto_alloc((int64_t)n + 1);
    size_t leido = fread(buf, 1, (size_t)n, f);
    fclose(f);
    buf[leido] = '\0';
    return (int64_t)(intptr_t)buf;
}

/* `_archivo_escribir(ruta, contenido)` → 0, o -1. */
int64_t forja_archivo_escribir(const char *ruta, const char *contenido) {
    if (!ruta || !*ruta) return -1;
    FILE *f = fopen(ruta, "wb");
    if (!f) return -1;
    size_t n = contenido ? strlen(contenido) : 0;
    size_t escrito = n ? fwrite(contenido, 1, n, f) : 0;
    fclose(f);
    return escrito == n ? 0 : -1;
}

/* `_archivo_agregar(ruta, contenido)` → 0, o -1. Crea el archivo si no existe. */
int64_t forja_archivo_agregar(const char *ruta, const char *contenido) {
    if (!ruta || !*ruta) return -1;
    FILE *f = fopen(ruta, "ab");
    if (!f) return -1;
    size_t n = contenido ? strlen(contenido) : 0;
    size_t escrito = n ? fwrite(contenido, 1, n, f) : 0;
    fclose(f);
    return escrito == n ? 0 : -1;
}

/* `stat` y no `fopen`: la VM (`Path::exists`) dice que existe para directorios. */
int64_t forja_archivo_existe(const char *ruta) {
    struct stat st;
    if (!ruta || !*ruta) return 0;
    return stat(ruta, &st) == 0 ? 1 : 0;
}

/* `_archivo_eliminar(ruta)` → 0, o -1. */
int64_t forja_archivo_eliminar(const char *ruta) {
    if (!ruta || !*ruta) return -1;
    return remove(ruta) == 0 ? 0 : -1;
}

/* `_archivo_tamano(ruta)` → bytes, o -1 (como `std::fs::metadata().len()`). */
int64_t forja_archivo_tamano(const char *ruta) {
    struct stat st;
    if (!ruta || !*ruta) return -1;
    if (stat(ruta, &st) != 0) return -1;
    return (int64_t)st.st_size;
}

/* `_sistema_variable_entorno(nombre)` → texto, o `nulo` (NULL). */
char *forja_env_obtener(const char *nombre) {
    if (!nombre) return NULL;
    const char *v = getenv(nombre);
    return v ? forja_dup(v) : NULL;
}

int64_t forja_env_existe(const char *nombre) {
    return nombre && getenv(nombre) ? 1 : 0;
}

void forja_env_establecer(const char *nombre, const char *valor) {
    if (!nombre) return;
#ifdef _WIN32
    _putenv_s(nombre, valor ? valor : "");
#else
    setenv(nombre, valor ? valor : "", 1);
#endif
}

/* En Windows `_putenv_s(nombre, "")` deja la variable *vacía* (definida), no
 * ausente: `shortcut:` a diferencia de la VM, `existe_variable` da verdadero
 * después de `eliminar`; alinear cuando Windows exponga `unsetenv`. */
void forja_env_eliminar(const char *nombre) {
    if (!nombre) return;
#ifdef _WIN32
    _putenv_s(nombre, "");
#else
    unsetenv(nombre);
#endif
}

/* `_sistema_so()`: el mismo nombre que `std::env::consts::OS`. */
char *forja_sistema_so(void) {
#ifdef _WIN32
    return forja_dup("windows");
#elif defined(__APPLE__)
    return forja_dup("macos");
#else
    return forja_dup("linux");
#endif
}

char *forja_sistema_directorio_actual(void) {
    char buf[4096];
    if (!getcwd(buf, sizeof buf)) return forja_dup("");
    return forja_dup(buf);
}

/* Directorio temporal sin separador final (la VM lo recorta igual). */
char *forja_sistema_directorio_temporal(void) {
    const char *t = getenv("TEMP");
    if (!t || !*t) t = getenv("TMP");
    if (!t || !*t) t = getenv("TMPDIR");
#ifndef _WIN32
    if (!t || !*t) t = "/tmp";
#endif
    if (!t || !*t) t = ".";
    size_t n = strlen(t);
        while (n > 1 && (t[n - 1] == '/' || t[n - 1] == (char)92)) n--;
    char *r = forja_texto_alloc((int64_t)n + 1);
    memcpy(r, t, n);
    r[n] = '\0';
    return r;
}

char *forja_sistema_hostname(void) {
    const char *h = getenv("COMPUTERNAME");
    if (!h || !*h) h = getenv("HOSTNAME");
    return forja_dup(h && *h ? h : "desconocido");
}

char *forja_sistema_usuario(void) {
    const char *u = getenv("USERNAME");
    if (!u || !*u) u = getenv("USER");
    return forja_dup(u ? u : "");
}

int64_t forja_sistema_cpus(void) {
#ifdef _WIN32
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    return (int64_t)si.dwNumberOfProcessors;
#else
    long n = sysconf(_SC_NPROCESSORS_ONLN);
    return n > 0 ? (int64_t)n : 1;
#endif
}

/* `_sistema_salir(codigo)`: volcar la salida antes de salir, como la VM. */
void forja_sistema_salir(int64_t codigo) {
    fflush(NULL);
    exit((int)codigo);
}
