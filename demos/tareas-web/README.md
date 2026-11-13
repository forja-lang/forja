# Demo: tareas (web real con Forja)

Backend HTTP + SQLite + sesiones + login + plantillas SSR. **Sin JavaScript**:
registro, login, alta de tareas, marcar hecha y logout funcionan con formularios
HTML comunes (y por eso la página es indexable y accesible).

```bash
cd demos/tareas-web
forja run main.fa
# abrir http://127.0.0.1:8080/
```

Configuración por entorno (no hace falta recompilar):

| Variable | Por defecto | Qué hace |
|---|---|---|
| `FORJA_WEB_PUERTO` | `8080` | puerto |
| `FORJA_WEB_BD` | `tareas.db` | archivo SQLite |
| `FORJA_WEB_SECRETO` | (uno de ejemplo) | clave con la que se firma la cookie `sid` |

## Qué demuestra

- **Migraciones**: `migraciones/001_inicial.sql` se aplica una sola vez
  (`std/datos.migrar` registra el nombre en `migraciones_aplicadas`).
- **Sesión**: cookie `sid` firmada con HMAC-SHA256, `HttpOnly`, `SameSite=Lax`,
  rotación de id al iniciar sesión.
- **CSRF**: cada formulario POST lleva un token derivado de la sesión y se
  verifica antes de tocar datos (`403` si falta o no coincide).
- **Contraseñas**: PBKDF2 (`_hash_password`), y el login verifica siempre un
  hash (aunque el usuario no exista) para no filtrar qué correos están dados de
  alta por tiempo de respuesta.
- **Rate limit**: 5 intentos por usuario y por IP cada 5 minutos (`429`).
- **SSR seguro**: el HTML se arma con `std/plantillas`; todo valor sale
  escapado (`{{…}}`), y un título con `<script>` se ve como texto.
- **Autorización por fila**: marcar una tarea hecha usa
  `WHERE id = ? AND usuario_id = ?`, así una tarea ajena no se puede tocar.
- **Estáticos**: `/publico` sirve el CSS con `ETag` y `304`.
- **Observabilidad**: `GET /salud` y `GET /metricas`, y una línea de log por
  request con método, ruta, código y milisegundos.

## Prueba automática

```bash
bash scripts/probar-demo-web.sh     # desde la raíz del repo
```

Levanta la demo con una base temporal y verifica todo el flujo con `curl`,
incluido que el CSRF sin token da `403` y que un título con `<script>` sale
escapado.

## Límites conocidos

- Un solo proceso y un servidor de un hilo por conexión: sirve para demos y
  tráfico bajo. El pool de hilos espera a que los hilos compartan estado (N11
  del plan web).
- Las sesiones viven en memoria: al reiniciar se pierden. El siguiente paso es
  guardarlas en Redis (`std/redis` ya tiene `HSET`/`GET`/`EXPIRE`).
- No hay subida de archivos (`multipart/form-data`) todavía.
