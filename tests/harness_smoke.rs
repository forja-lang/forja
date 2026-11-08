//! Verifica que el harness compartido compila y ejecuta casos reales.

use forja_test_support::{caso, falla, parsea};

parsea!(parsea_variable, "variable x = 1\nescribir(x)\n");

caso!(ejecuta_suma, "escribir(2 + 3)\n", vec!["5"]);

falla!(
    falla_variable_inexistente,
    "escribir(no_existe)\n",
    "no_existe"
);
