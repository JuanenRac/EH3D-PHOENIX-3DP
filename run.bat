@echo off
rem Runs phoenix-3dp with its sample. Extra arguments replace the sample: run.bat [ARGUMENTS]
cd /d "%~dp0"
set "ARGS=%*"
if "%ARGS%"=="" set "ARGS=info examples/cube-20mm.stl --printer bambu-lab-a1"
cargo run --release --quiet -- %ARGS%
if not defined NOPAUSE pause
exit /b %ERRORLEVEL%
