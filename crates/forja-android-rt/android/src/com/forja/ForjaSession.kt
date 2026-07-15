package com.forja

import java.util.function.Consumer
import java.util.function.Supplier

/**
 * Una sesión de Forja con estado persistente.
 *
 * Las variables, funciones y clases definidas en llamadas anteriores
 * a [ejecutar] se mantienen disponibles en llamadas posteriores.
 *
 * @property nativePtr Puntero opaco a la sesión nativa en Rust.
 */
class ForjaSession internal constructor(
    internal val nativePtr: Long
) : AutoCloseable {

    /**
     * Compila y ejecuta código fuente Forja en esta sesión.
     * El estado (variables, funciones) persiste entre llamadas.
     *
     * @param source Código fuente Forja a ejecutar.
     * @param rutaBase Ruta base para resolución de imports.
     * @return Resultado con output, estadísticas y posible error.
     */
    @JvmOverloads
    fun ejecutar(source: String, rutaBase: String = ""): ForjaResult {
        return ForjaRuntime.nativeEjecutar(nativePtr, source, rutaBase)
    }

    /**
     * Compila código Forja a bytecode serializado.
     * Útil para pre-compilar y ejecutar después sin re-compilar.
     *
     * @param source Código fuente Forja.
     * @return Bytecode serializado en formato .fbc.
     */
    fun compilarABytecode(source: String): ByteArray {
        return ForjaRuntime.nativeCompilarABytecode(nativePtr, source)
    }

    /**
     * Ejecuta bytecode pre-compilado en esta sesión.
     *
     * @param bytecode Bytecode obtenido de [compilarABytecode].
     * @return Resultado de la ejecución.
     */
    fun ejecutarBytecode(bytecode: ByteArray): ForjaResult {
        return ForjaRuntime.nativeEjecutarBytecode(nativePtr, bytecode)
    }

    /**
     * Evalúa una expresión Forja y devuelve su valor como Object.
     *
     * Útil para REPL o para obtener valores calculados desde Java/Kotlin.
     *
     * @param expresion Expresión Forja a evaluar (ej: "2 + 2", "nombre").
     * @return Valor resultante (Long, Double, String, Boolean, List, Map, etc.)
     */
    fun evaluar(expresion: String): Any? {
        return ForjaRuntime.nativeEvaluar(nativePtr, expresion)
    }

    /**
     * Registra un callback que se invocará cada vez que el script
     * use `escribir()`.
     *
     * @param callback Función que recibe cada línea de output.
     */
    fun setOutputCallback(callback: ((String) -> Unit)?) {
        val consumer = if (callback != null) {
            Consumer<String> { callback(it) }
        } else null
        ForjaRuntime.nativeSetOutputCallback(nativePtr, consumer)
    }

    /**
     * Registra un callback que se invocará cuando el script
     * use `leer()` para solicitar entrada del usuario.
     *
     * @param callback Función que retorna el texto ingresado por el usuario.
     */
    fun setInputCallback(callback: (() -> String)?) {
        val supplier = if (callback != null) {
            Supplier<String> { callback() }
        } else null
        ForjaRuntime.nativeSetInputCallback(nativePtr, supplier)
    }

    /**
     * Resetea el estado de la sesión (variables, output, stack),
     * pero conserva la configuración (callbacks, límites).
     */
    fun reset() {
        ForjaRuntime.nativeResetSession(nativePtr)
    }

    /**
     * Destruye la sesión y libera todos los recursos nativos.
     * Una vez destruida, la sesión no puede usarse más.
     */
    fun destruir() {
        ForjaRuntime.nativeDestruirSession(nativePtr)
    }

    /**
     * AutoCloseable: permite usar con `use {}`.
     */
    override fun close() {
        destruir()
    }
}
