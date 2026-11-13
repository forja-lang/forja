-- 001_inicial.sql — esquema de la demo "tareas".
-- Se aplica una sola vez: el nombre del archivo queda en `migraciones_aplicadas`.

CREATE TABLE IF NOT EXISTS usuarios (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    email    TEXT NOT NULL UNIQUE,
    hash     TEXT NOT NULL,
    creado   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS tareas (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    usuario_id INTEGER NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE,
    titulo     TEXT NOT NULL,
    hecha      INTEGER NOT NULL DEFAULT 0,
    creada     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tareas_usuario ON tareas (usuario_id, hecha);
