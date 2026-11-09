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

void *forja_alloc(int64_t bytes) {
    void *p = calloc(1, (size_t)(bytes > 0 ? bytes : 1));
    if (!p) {
        fputs("forja: sin memoria\n", stderr);
        exit(1);
    }
    return p;
}

/* ── tiempo ───────────────────────────────────────────────────────────── */

int64_t forja_tiempo_ms(void) {
#ifdef _WIN32
    return (int64_t)GetTickCount64();
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
void forja_imprimir_nulo(void) { fputs("nulo", stdout); }
void forja_nueva_linea(void) { fputc('\n', stdout); }

/* ── arreglos ─────────────────────────────────────────────────────────── */
/* Los arreglos de Forja son heterogéneos: cada casilla guarda un i64 (los
 * textos viajan como puntero convertido con ptrtoint). */
typedef struct {
    int64_t len;
    int64_t cap;
    int64_t *datos;
} ForjaArreglo;

void *forja_arreglo_nuevo(int64_t n) {
    ForjaArreglo *a = (ForjaArreglo *)forja_alloc((int64_t)sizeof(ForjaArreglo));
    a->len = n;
    a->cap = n > 4 ? n : 4;
    a->datos = (int64_t *)forja_alloc(a->cap * (int64_t)sizeof(int64_t));
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
    if (!a || i < 0 || i >= a->len) {
        fprintf(stderr, "forja: índice %lld fuera de rango (longitud %lld)\n", (long long)i,
                (long long)(a ? a->len : 0));
        exit(1);
    }
    a->datos[i] = v;
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

void forja_mapa_fijar_i64(void *pm, int64_t clave, int64_t valor) {
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
}

int64_t forja_mapa_obtener_i64(void *pm, int64_t clave) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m || m->len == 0) return 0;
    ForjaPar *p = forja_mapa_slot(m, forja_hash_i64(clave), clave, NULL);
    return p->usado ? p->valor : 0;
}

void forja_mapa_fijar_texto(void *pm, const char *clave, int64_t valor) {
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
}

int64_t forja_mapa_obtener_texto(void *pm, const char *clave) {
    ForjaMapa *m = (ForjaMapa *)pm;
    if (!m || m->len == 0) return 0;
    ForjaPar *p = forja_mapa_slot(m, forja_hash_texto(clave), -1, clave);
    return p->usado ? p->valor : 0;
}
