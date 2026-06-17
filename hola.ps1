# Ejecutable generado por Forja
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
& "$scriptDir\forja.exe" run "$scriptDir\hola.fbc" @args
