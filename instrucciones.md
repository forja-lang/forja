# 📖 Instrucciones de Forja (fa)

**Forja** es un lenguaje de programación en español con sintaxis clara, tipos dinámicos, y un poderoso sistema de ownership. Esta guía cubre **todas las palabras clave** con ejemplos prácticos.

---

## Índice de Palabras Clave

| # | Palabra | Alias | Tipo | Propósito |
|---|---------|:----:|:----:|-----------|
| 1 | [`variable`](#1-variable) | `var` | Declaración | Variable mutable |
| 2 | [`constante`](#2-constante) | `const` | Declaración | Variable inmutable |
| 3 | [`escribir`](#3-escribir) | Builtin | Mostrar en pantalla |
| 4 | [`leer`](#4-leer) | Builtin | Leer entrada del usuario |
| 5 | [`si` / `sino`](#5-si--sino) | Control | Condicionales |
| 6 | [`mientras`](#6-mientras) | Control | Bucle while |
| 7 | [`para`](#7-para) | Control | Bucle for |
| 8 | [`repetir`](#8-repetir) | Control | Bucle de repetición fija |
| 9 | [`funcion`](#9-funcion) | `fun` | Declaración | Definir función |
| 10 | [`retornar`](#10-retornar) | Control | Devolver valor |
| 11 | [`clase`](#11-clase) | Declaración | Definir clase |
| 12 | [`constructor`](#12-constructor) | Declaración | Inicializador de clase |
| 13 | [`nuevo`](#13-nuevo) | Expresión | Instanciar objeto |
| 14 | [`este`](#14-este) | Expresión | Referencia al objeto actual |
| 15 | [`importar`](#15-importar) | Módulos | Importar módulos |
| 16 | [`verdadero` / `falso`](#16-verdadero--falso) | Literal | Valores booleanos |
| 17 | [`nulo`](#17-nulo) | Literal | Ausencia de valor |
| 18 | [`coincidir` / `caso`](#18-coincidir--caso) | Control | Pattern matching |
| 19 | [`tipo`](#19-tipo) | Declaración | Tipos algebraicos (enums) |
| 20 | [`prestado`](#20-prestado) | Declaración | Parámetro por referencia |
| 21 | [`&` (referencia)](#21---referencia) | Operador | Préstamo de variable |
| 22 | [`Texto` / `Entero` / `Decimal` / `Booleano`](#22-texto--entero--decimal--booleano) | Tipo | Anotaciones de tipo |

---

## 1. `variable` / `var`

Declara una variable **mutable** (se puede reasignar). Es como `let mut` en Rust.

> **Alias**: podés usar `var` en lugar de `variable`.

```
variable nombre = "Ana"
variable edad = 25

edad = 26  // ✅ Se puede cambiar
```

```
variable contador = 0
mientras (contador < 5) {
    escribir(contador)
    contador = contador + 1  // mutable: se puede modificar
}
```

---

## 2. `constante` / `const`

Declara una variable **inmutable** (no se puede reasignar). Es como `let` en Rust.

> **Alias**: podés usar `const` en lugar de `constante`.

```
constante pais = "Argentina"
escribir(pais)

// pais = "Chile"  // ❌ ERROR: constante no se puede modificar
```

```
constante IVA = 21
variable precio = 100
variable total = precio + (precio * IVA / 100)
escribir(total)  // 121
```

---

## 3. `escribir`

Muestra texto, números o cualquier valor en la terminal. Función builtin.

```
escribir("Hola, mundo!")
escribir(42)
escribir(3.14)
escribir(verdadero)
```

```
variable nombre = "Pedro"
variable edad = 30
escribir("Me llamo " + nombre + " y tengo " + edad + " años")
```

---

## 4. `leer`

Lee una línea de texto ingresada por el usuario. **Siempre devuelve Texto**.

```
escribir("¿Cómo te llamás?")
variable nombre = leer()
escribir("¡Hola, " + nombre + "!")
```

```
escribir("Decí un número:")
variable entrada = leer()
escribir("Escribiste: " + entrada)
```

---

## 5. `si` / `sino`

Condicional: ejecuta un bloque si la condición es verdadera, y opcionalmente otro bloque si es falsa.

```
variable edad = 18
si (edad >= 18) {
    escribir("Sos mayor de edad")
} sino {
    escribir("Sos menor")
}
```

```
variable nota = 85
si (nota >= 90) {
    escribir("Excelente!")
} sino {
    si (nota >= 70) {
        escribir("Buen trabajo!")
    } sino {
        escribir("Seguí estudiando!")
    }
}
```

---

## 6. `mientras`

Bucle que se repite **mientras** la condición sea verdadera.

```
variable i = 0
mientras (i < 3) {
    escribir("Vuelta: " + i)
    i = i + 1
}
// Imprime: Vuelta: 0, Vuelta: 1, Vuelta: 2
```

```
variable s = 0
variable i = 0
mientras (i < 10000) {
    s = s + i
    i = i + 1
}
escribir(s)  // 49995000
```

---

## 7. `para`

Bucle con inicialización, condición e incremento en una sola línea.

```
para (variable i = 0; i < 5; i = i + 1) {
    escribir("i = " + i)
}
// Imprime: i = 0, i = 1, ..., i = 4
```

```
para (variable i = 1; i <= 10; i = i + 1) {
    escribir(i * i)  // cuadrados: 1, 4, 9, ..., 100
}
```

---

## 8. `repetir`

Bucle que se repite **una cantidad fija de veces**.

```
repetir (3) {
    escribir("Esto se repite 3 veces")
}
```

```
variable suma = 0
repetir (10) {
    suma = suma + 1
}
escribir(suma)  // 10
```

---

## 9. `funcion` / `fun`

Define una función reutilizable. Puede recibir parámetros y devolver un valor con `retornar`.

> **Alias**: también podés escribir `fun` en lugar de `funcion`.

```
funcion saludar(nombre) {
    escribir("Hola, " + nombre + "!")
}

saludar("Ana")
saludar("Pedro")
```

```
funcion suma(a, b) {
    retornar a + b
}

funcion factorial(n) {
    si (n <= 1) { retornar 1 }
    retornar n * factorial(n - 1)
}

escribir(suma(5, 3))      // 8
escribir(factorial(5))    // 120
```

---

## 10. `retornar`

Devuelve un valor desde una función y termina su ejecución.

```
funcion fibonacci(n) {
    si (n <= 1) { retornar n }
    variable a = 0
    variable b = 1
    variable i = 2
    mientras (i <= n) {
        variable t = a + b
        a = b
        b = t
        i = i + 1
    }
    retornar b
}

escribir(fibonacci(30))  // 832040
```

---

## 11. `clase`

Define una **clase** (plantilla para crear objetos). Puede tener campos y métodos.

```
clase Persona {
    nombre
    edad

    constructor(n, e) {
        este.nombre = n
        este.edad = e
    }

    funcion presentarse() {
        escribir("Hola, soy " + este.nombre + " y tengo " + este.edad + " años")
    }
}
```

```
clase Punto {
    x
    y
}
// Sin constructor — los campos se asignan directamente
variable p = nuevo Punto()
p.x = 10
p.y = 20
escribir(p.x + p.y)  // 30
```

---

## 12. `constructor`

Método especial que se ejecuta automáticamente al crear un objeto con `nuevo`. Inicializa los campos.

```
clase Libro {
    titulo
    autor

    constructor(t, a) {
        este.titulo = t
        este.autor = a
    }

    funcion mostrar() {
        escribir(este.titulo + " por " + este.autor)
    }
}

variable libro = nuevo Libro("Cien Años de Soledad", "García Márquez")
libro.mostrar()
```

---

## 13. `nuevo`

Crea una **instancia** de una clase. Ejecuta el `constructor` si existe.

```
clase Perro {
    nombre
    raza
    constructor(n, r) {
        este.nombre = n
        este.raza = r
    }
    funcion ladrar() {
        escribir(este.nombre + " dice: Guau!")
    }
}

variable perro1 = nuevo Perro("Fido", "Labrador")
variable perro2 = nuevo Perro("Rex", "Pastor")
perro1.ladrar()
perro2.ladrar()
```

---

## 14. `este`

Hace referencia al **objeto actual** dentro de un método de clase. Equivalente a `self` o `this`.

```
clase Contador {
    valor

    constructor() {
        este.valor = 0
    }

    funcion incrementar() {
        este.valor = este.valor + 1
    }

    funcion obtener() {
        retornar este.valor
    }
}

variable c = nuevo Contador()
c.incrementar()
c.incrementar()
escribir(c.obtener())  // 2
```

---

## 15. `importar`

Importa funciones y clases desde otro archivo Forja.

```
// archivo: calculos.fa
funcion suma(a, b) { retornar a + b }
funcion resta(a, b) { retornar a - b }
```

```
// archivo: main.fa
importar calculos

escribir(suma(10, 5))   // 15
escribir(resta(10, 5))  // 5
```

---

## 16. `verdadero` / `falso`

Valores **booleanos**: representan verdadero o falso.

```
variable activo = verdadero
variable terminado = falso

si (activo) {
    escribir("Está activo")
}

si (5 > 3) {            // verdadero
    escribir("5 es mayor que 3")
}
```

---

## 17. `nulo`

Representa la **ausencia de valor**. Similar a `null` o `None`.

```
variable resultado = nulo
escribir(resultado)  // nulo

funcion buscar(id) {
    // retornar nulo si no se encuentra
    retornar nulo
}
```

---

## 18. `coincidir` / `caso`

**Pattern matching**: compara un valor contra múltiples patrones y ejecuta el primero que coincida.

```
variable dia = 3

coincidir (dia) {
    caso 1 { escribir("Lunes") }
    caso 2 { escribir("Martes") }
    caso 3 { escribir("Miércoles") }
    caso 4 { escribir("Jueves") }
    caso 5 { escribir("Viernes") }
    caso _ { escribir("Fin de semana") }
}
// Imprime: Miércoles
```

```
variable comando = "salir"

coincidir (comando) {
    caso "ayuda" { escribir("Mostrando ayuda...") }
    caso "salir" { escribir("Chau!") }
    caso _ { escribir("Comando no reconocido") }
}
// El caso _ (comodín) atrapa cualquier valor no listado
```

---

## 19. `tipo`

Define **tipos algebraicos** (enums): un tipo que puede ser uno de varios valores posibles.

```
tipo Resultado = Exito(Entero) | Error(Texto)

// Los constructores 'Exito' y 'Error' se usan como funciones:
// Exito(200)
// Error("archivo no encontrado")
```

```
tipo Color = Rojo | Verde | Azul | Personalizado(Entero, Entero, Entero)

// Rojo, Verde, Azul son variantes sin datos
// Personalizado(r, g, b) lleva tres números adjuntos
```

---

## 20. `prestado`

Marca un parámetro de función como **prestado por referencia** (no se toma ownership). Similar a `&` en Rust.

```
funcion mostrar(prestado texto) {
    escribir(texto)
    // No puede modificar 'texto' porque es prestado
}

variable saludo = "Hola!"
mostrar(saludo)       // 'saludo' se presta, no se mueve
escribir(saludo)      // ✅ todavía accesible
```

---

## 21. `&` (referencia)

Crea una **referencia** (préstamo) a una variable. No toma ownership.

```
variable x = 42
variable ref = &x
escribir("Valor de x: " + x)   // x sigue siendo accesible

funcion mostrar(prestado val) {
    escribir("El valor es: " + val)
}

variable dato = 100
mostrar(&dato)  // prestamos 'dato' sin moverlo
escribir(dato)  // ✅ sigue disponible
```

---

## 22. `Texto` / `Entero` / `Decimal` / `Booleano`

Anotaciones de **tipo explícito** para variables y parámetros.

```
// Con tipo explícito
variable nombre: Texto = "Gaucho"
variable edad: Entero = 30
variable altura: Decimal = 1.85
variable activo: Booleano = verdadero

// Sin tipo (inferido automáticamente)
variable ciudad = "Buenos Aires"  // Forja infiere: Texto
variable total = 42               // Forja infiere: Entero
```

```
funcion duplicar(n: Entero) -> Entero {
    retornar n * 2
}

funcion saludar(nombre: Texto) {
    escribir("Hola " + nombre)
}
```

---

## 📐 Operadores

| Operador | Propósito | Ejemplo | Resultado |
|:--------:|-----------|---------|:---------:|
| `+` | Suma / concatenación | `10 + 3` / `"a" + "b"` | `13` / `"ab"` |
| `-` | Resta | `10 - 3` | `7` |
| `*` | Multiplicación | `10 * 3` | `30` |
| `/` | División | `10 / 3` | `3` (entera) |
| `==` | Igualdad | `5 == 3` | `falso` |
| `!=` | Diferente | `5 != 3` | `verdadero` |
| `>` | Mayor que | `5 > 3` | `verdadero` |
| `<` | Menor que | `5 < 3` | `falso` |
| `>=` | Mayor o igual | `5 >= 5` | `verdadero` |
| `<=` | Menor o igual | `5 <= 3` | `falso` |
| `&&` | Y lógico | `verdadero && falso` | `falso` |
| `\|\|` | O lógico | `verdadero \|\| falso` | `verdadero` |
| `!` | NO lógico | `!verdadero` | `falso` |

---

## 📦 Colecciones

### Arreglos (Listas)
```
variable frutas = ["manzana", "banana", "naranja"]
escribir(frutas[0])       // manzana
frutas[1] = "pera"
escribir(frutas.length()) // 3
```

### Mapas (Diccionarios)
```
variable persona = {"nombre": "Ana", "edad": 25}
escribir(persona["nombre"])  // Ana
persona["edad"] = 26
```

---

## 🧵 Métodos de Texto

Los strings tienen métodos incorporados:

```
variable texto = "  Hola, Forja!  "
escribir(texto.length())          // 15
escribir(texto.trim())            // "Hola, Forja!"
escribir(texto.to_upper())        // "  HOLA, FORJA!  "
escribir(texto.to_lower())        // "  hola, forja!  "
escribir(texto.contains("Forja")) // verdadero
```

---

## 🎯 Ejemplo Completo: Calculadora

```
funcion factorial(n) {
    si (n <= 1) { retornar 1 }
    retornar n * factorial(n - 1)
}

funcion fibonacci(n) {
    si (n <= 1) { retornar n }
    variable a = 0
    variable b = 1
    variable i = 2
    mientras (i <= n) {
        variable t = a + b
        a = b
        b = t
        i = i + 1
    }
    retornar b
}

escribir("Factorial de 5: " + factorial(5))     // 120
escribir("Fibonacci 30: " + fibonacci(30))      // 832040

variable suma = 0
para (variable i = 1; i <= 100; i = i + 1) {
    suma = suma + i
}
escribir("Suma 1..100: " + suma)                // 5050
```

---

> 📌 **Para ejecutar**: `forja run instrucciones.md` o `cargo run -- run instrucciones.fa`
