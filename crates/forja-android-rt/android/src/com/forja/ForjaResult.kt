package com.forja

/**
 * Resultado de una ejecución de Forja.
 *
 * @property output Líneas de output producidas por `escribir()`.
 * @property ejecutadas Cantidad de instrucciones ejecutadas.
 * @property duracionNs Duración de la ejecución en nanosegundos.
 * @property error Error ocurrido durante la ejecución (null si fue exitosa).
 */
data class ForjaResult(
    val output: List<String>,
    val ejecutadas: Long,
    val duracionNs: Long,
    val error: ForjaError? = null
) {
    companion object {
        /** Crea un resultado exitoso. */
        fun exito(
            output: List<String> = emptyList(),
            ejecutadas: Long = 0,
            duracionNs: Long = 0
        ) = ForjaResult(output, ejecutadas, duracionNs, null)

        /** Crea un resultado con error. */
        fun error(
            error: ForjaError,
            output: List<String> = emptyList(),
            ejecutadas: Long = 0,
            duracionNs: Long = 0
        ) = ForjaResult(output, ejecutadas, duracionNs, error)
    }

    /** True si la ejecución fue exitosa. */
    val esExito: Boolean get() = error == null

    /** Duración en milisegundos. */
    val duracionMs: Double get() = duracionNs / 1_000_000.0

    /** Output completo como un solo string. */
    val texto: String get() = output.joinToString("\n")
}

/**
 * Error estructurado de Forja.
 *
 * @property mensaje Descripción del error.
 * @property linea Línea donde ocurrió el error (0 si no aplica).
 * @property columna Columna donde ocurrió el error (0 si no aplica).
 * @property tipo Tipo de error (COMPILE, RUNTIME, CONTRACT, TIMEOUT, INTERNAL).
 */
data class ForjaError(
    val mensaje: String,
    val linea: Int,
    val columna: Int,
    val tipo: String
) {
    override fun toString(): String {
        val loc = if (linea > 0) " [$linea:$columna]" else ""
        return "[$tipo$loc] $mensaje"
    }
}

/**
 * Representa un objeto de Forja en el mundo Java.
 * Similar a un Map<String, Object> pero con nombre de clase.
 *
 * @property className Nombre de la clase Forja.
 * @property fields Mapa de campos del objeto.
 */
data class ForjaObject(
    val className: String,
    val fields: Map<String, Any?>
)
