package com.forja.app

import android.os.Bundle
import android.widget.TextView
import android.widget.ScrollView
import androidx.appcompat.app.AppCompatActivity
import com.forja.ForjaRuntime
import com.forja.ForjaSession
import java.io.InputStream

/**
 * Activity principal que carga y ejecuta bytecode Forja desde assets/main.fbc
 * o código fuente desde assets/main.fa.
 *
 * El usuario escribe su app en Forja, y esta Activity la ejecuta.
 * No necesita escribir nada en Kotlin/Java.
 */
class MainActivity : AppCompatActivity() {

    private lateinit var outputText: TextView
    private lateinit var scrollView: ScrollView
    private var session: ForjaSession? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Layout: ScrollView con TextView de output
        outputText = TextView(this).apply {
            textSize = 14f
            setTextColor(-0x1f1f20) // blanco hueso
            setBackgroundColor(-0xe1e1e2) // fondo oscuro
            setPadding(24, 24, 24, 24)
            typeface = android.graphics.Typeface.MONOSPACE
            text = "🧪 Forja • Cargando..."
        }
        scrollView = ScrollView(this).apply { addView(outputText) }
        setContentView(scrollView)
        supportActionBar?.hide()

        // Ejecutar en background thread para no bloquear UI
        Thread { ejecutarForja() }.start()
    }

    private fun ejecutarForja() {
        try {
            // 1. Intentar cargar bytecode compilado (.fbc)
            val bytecode = cargarAsset("main.fbc")
            if (bytecode != null) {
                output("⚡ Ejecutando bytecode compilado...")
                session = ForjaRuntime.crearSession()
                val result = session!!.ejecutarBytecode(bytecode)
                mostrarResultado(result)
                return
            }

            // 2. Fallback: cargar código fuente (.fa) y compilar
            val source = cargarAssetTexto("main.fa")
            if (source != null) {
                output("🔨 Compilando y ejecutando main.fa...")
                session = ForjaRuntime.crearSession()
                val result = session!!.ejecutar(source)
                mostrarResultado(result)
                return
            }

            // 3. Si no hay ni fbc ni fa, mostrar ayuda
            output("""
                ⚠️  No se encontró main.fbc ni main.fa
                
                Creá un archivo app/src/main/assets/main.fa con tu código Forja.
                
                Ejemplo:
                ─────────────────────────────────
                escribir("Hola desde Forja en Android!")
                
                variable numeros = arreglo[]
                para (variable i = 1; i <= 10; i = i + 1) {
                    numeros.empujar(i)
                }
                escribir("Suma: " + numeros[0] + numeros[1])
                ─────────────────────────────────
                
                Luego ejecutá:
                  forja construir-apk
            """.trimIndent())
        } catch (e: Exception) {
            output("❌ Error: ${e.message}")
            e.printStackTrace()
        }
    }

    private fun mostrarResultado(result: com.forja.ForjaResult) {
        if (result.esExito) {
            for (line in result.output) {
                output(line)
            }
            output("")
            output("✅ Hecho • ${result.duracionMs} ms • ${result.ejecutadas} instr")
        } else {
            output("❌ ${result.error}")
        }
    }

    private fun cargarAsset(nombre: String): ByteArray? {
        return try {
            assets.open(nombre).use { it.readBytes() }
        } catch (_: Exception) { null }
    }

    private fun cargarAssetTexto(nombre: String): String? {
        return try {
            assets.open(nombre).bufferedReader().use { it.readText() }
        } catch (_: Exception) { null }
    }

    private fun output(texto: String) {
        runOnUiThread {
            outputText.append(texto + "\n")
            scrollView.post { scrollView.fullScroll(ScrollView.FOCUS_DOWN) }
        }
    }
}
