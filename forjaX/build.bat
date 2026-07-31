@echo off
REM ============================================================
REM  forjaX - inyector X-Ray en Forja (port de cs2-rayoX)
REM
REM  Uso:
REM    build.bat          -> ejecuta el inyector (requiere CS2
REM                          abierto y ejecutar como Administrador)
REM    build.bat compilar -> genera forjaX.exe autonomo
REM ============================================================
setlocal
cd /d "%~dp0"

REM Buscar el compilador de Forja (release primero, debug como fallback)
set "FORJA=..\target\release\forja.exe"
if not exist "%FORJA%" set "FORJA=..\target\debug\forja.exe"
if not exist "%FORJA%" (
    echo [ERROR] No se encuentra el compilador de Forja.
    echo Compilalo primero:
    echo   cd C:\Users\gaucho\forja
    echo   cargo build --release --bin forja
    exit /b 1
)

if "%1"=="compilar" (
    echo [OK] Compilando forjaX.exe ...
    "%FORJA%" compilar main.fa -o forjaX.exe
    exit /b %errorlevel%
)

echo [OK] Ejecutando inyector con: %FORJA%
"%FORJA%" ejecutar main.fa
