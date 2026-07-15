@echo off
setlocal EnableExtensions

set "LIB_NAME=point_glass_opengl_core"

set "SCRIPT_DIR=%~dp0"
set "TARGET_DIR=%SCRIPT_DIR%target\release"
set "OUTPUT_DIR=%SCRIPT_DIR%..\windows\shared"

set "SHARED_LIB=%LIB_NAME%.dll"

cd /d "%SCRIPT_DIR%"

cargo build --release
if errorlevel 1 (
    echo [ERROR] Cargo build failed.
    exit /b 1
)

if not exist "%OUTPUT_DIR%" (
    mkdir "%OUTPUT_DIR%"
)

call :copy_artifact "%SHARED_LIB%"
if errorlevel 1 exit /b 1

echo [INFO] Build completed:
echo        %OUTPUT_DIR%

endlocal
exit /b 0

:copy_artifact
set "FILE_NAME=%~1"
set "SOURCE_FILE=%TARGET_DIR%\%FILE_NAME%"

if not exist "%SOURCE_FILE%" (
    echo [ERROR] Build artifact not found:
    echo         %SOURCE_FILE%
    exit /b 1
)

copy /y "%SOURCE_FILE%" "%OUTPUT_DIR%\%FILE_NAME%" >nul
if errorlevel 1 (
    echo [ERROR] Failed to copy:
    echo         %SOURCE_FILE%
    exit /b 1
)

echo [INFO] Copied %FILE_NAME%
exit /b 0