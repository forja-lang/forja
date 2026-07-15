package com.forja

import java.util.concurrent.Executors
import java.util.function.Consumer
import java.util.function.Supplier

/**
 * Runtime de Forja para Android.
 *
 * Permite compilar y ejecutar código Forja (.fa) desde aplicaciones Android.
 * Usa JNI para comunicarse con la VM de Forja implementada en Rust.
 *
 * Uso básico:
 * ```kotlin
 * val session = ForjaRuntime.crearSession()
 * val result = session.ejecutar("escribir("Hola Android!")")
 * Log.d("Forja", result.output.join("\n"))
 * session.destruir()
 * ```
 */
object ForjaRuntime {

    private val executor = Executors.newCachedThreadPool()

    init {
        System.loadLibrary("forja_android_rt")
    }

    // ─── Información de versión ───────────────────────────────────

    /** Retorna la versión del runtime Android de Forja. */
    external fun nativeVersion(): String

    /** Versión amigable del runtime. */
    fun version(): String = nativeVersion()

    // ─── Sesiones ─────────────────────────────────────────────────

    /**
     * Crea una nueva sesión de Forja con estado persistente.
     * Las sesiones mantienen variables y funciones entre llamadas a ejecutar().
     *
     * @param maxInst Límite máximo de instrucciones por ejecución (default: 10M).
     * @return Una nueva [ForjaSession].
     */
    @JvmOverloads
    fun crearSession(maxInst: Long = 10_000_000L): ForjaSession {
        val ptr = nativeCrearSession(maxInst)
        return ForjaSession(ptr)
    }

    private external fun nativeCrearSession(maxInst: Long): Long
    internal external fun nativeDestruirSession(sessionPtr: Long)
    internal external fun nativeResetSession(sessionPtr: Long)

    // ─── Ejecución one-shot ───────────────────────────────────────

    /**
     * Ejecuta código Forja de forma síncrona (crea sesión, ejecuta, destruye).
     * Bloquea el hilo actual hasta que termina la ejecución.
     *
     * @param source Código fuente Forja a ejecutar.
     * @param rutaBase Ruta base para resolución de imports.
     * @return Resultado de la ejecución.
     */
    @JvmOverloads
    fun ejecutar(source: String, rutaBase: String = ""): ForjaResult {
        val session = crearSession()
        return try {
            session.ejecutar(source, rutaBase)
        } finally {
            session.destruir()
        }
    }

    /**
     * Ejecuta código Forja de forma asíncrona.
     * No bloquea el hilo actual. Los callbacks se ejecutan en el hilo principal (UI).
     *
     * @param source Código fuente Forja.
     * @param onResult Callback con el resultado exitoso.
     * @param onError Callback con el error (opcional).
     * @param rutaBase Ruta base para imports.
     */
    @JvmOverloads
    fun ejecutarAsync(
        source: String,
        onResult: (ForjaResult) -> Unit,
        onError: ((ForjaError) -> Unit)? = null,
        rutaBase: String = ""
    ) {
        executor.submit {
            try {
                val result = ejecutar(source, rutaBase)
                android.os.Handler(android.os.Looper.getMainLooper()).post {
                    onResult(result)
                }
            } catch (e: ForjaException) {
                android.os.Handler(android.os.Looper.getMainLooper()).post {
                    onError?.invoke(e.error)
                }
            } catch (e: Exception) {
                android.os.Handler(android.os.Looper.getMainLooper()).post {
                    onError?.invoke(
                        ForjaError("Error inesperado: ${e.message}", 0, 0, "INTERNAL")
                    )
                }
            }
        }
    }

    // ─── JNI exports ──────────────────────────────────────────────

    internal external fun nativeEjecutar(
        sessionPtr: Long,
        source: String,
        rutaBase: String
    ): ForjaResult

    internal external fun nativeCompilarABytecode(
        sessionPtr: Long,
        source: String
    ): ByteArray

    internal external fun nativeEjecutarBytecode(
        sessionPtr: Long,
        bytecode: ByteArray
    ): ForjaResult

    internal external fun nativeEvaluar(
        sessionPtr: Long,
        expresion: String
    ): Any?

    internal external fun nativeSetOutputCallback(
        sessionPtr: Long,
        callback: Consumer<String>?
    )

    internal external fun nativeSetInputCallback(
        sessionPtr: Long,
        callback: Supplier<String>?
    )
}

/**
 * Excepción base para errores de Forja en Android.
 * Todas las excepciones lanzadas por el runtime Forja extienden esta clase.
 */
open class ForjaException(
    message: String,
    cause: Throwable? = null
) : RuntimeException(message, cause) {
    /** Obtiene el error estructurado asociado. */
    open val error: ForjaError
        get() = ForjaError(message ?: "Error desconocido", 0, 0, "RUNTIME")
}

/** Error de compilación (sintaxis, tipos, semántica). */
class ForjaCompileError(message: String) : ForjaException(message)

/** Error en tiempo de ejecución. */
class ForjaRuntimeError(message: String) : ForjaException(message) {
    override val error: ForjaError
        get() = ForjaError(message ?: "Error runtime", 0, 0, "RUNTIME")
}

/** Error de contrato (pre/post condición). */
class ForjaContractError(message: String) : ForjaException(message)

/** Timeout de ejecución. */
class ForjaTimeoutError(message: String) : ForjaException(message)

/** Error interno del runtime (bug). */
class ForjaInternalError(message: String) : ForjaException(message)
