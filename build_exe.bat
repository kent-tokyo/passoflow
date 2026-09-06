@echo off
REM Build a standalone --onedir distribution of passoflow into dist\passoflow\passoflow.exe.

cd /d "%~dp0"

pushd web-ui
call npm ci
if errorlevel 1 (popd & goto :error)
call npm run build
if errorlevel 1 (popd & goto :error)
popd

pip install -r requirements.txt pyinstaller
if errorlevel 1 goto :error

pyinstaller passoflow.spec
if errorlevel 1 goto :error

echo.
echo Build complete: dist\passoflow\passoflow.exe
pause
exit /b 0

:error
echo.
echo Build failed.
pause
exit /b 1
