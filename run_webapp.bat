@echo off
setlocal EnableExtensions

rem PassoFlow one-click launcher for Windows.
rem In a source checkout it starts FastAPI and Vite; in a packaged build it
rem starts FastAPI only because the built UI is served by the API.
cd /d "%~dp0"

set "PYTHON_BIN=python"
if exist "%~dp0.venv\Scripts\python.exe" (
  set "PYTHON_BIN=%~dp0.venv\Scripts\python.exe"
  goto :python_ready
)

where python >nul 2>&1
if errorlevel 1 goto :python_error

:python_ready
"%PYTHON_BIN%" -c "import fastapi, uvicorn, yaml" >nul 2>&1
if errorlevel 1 (
  echo Installing PassoFlow Python dependencies...
  "%PYTHON_BIN%" -m pip install -r requirements.txt
  if errorlevel 1 goto :dependency_error
)

if exist "%~dp0web-ui\dist\index.html" goto :start_packaged

where npm >nul 2>&1
if errorlevel 1 goto :node_error
if not exist "%~dp0web-ui\node_modules" (
  echo Installing PassoFlow web UI dependencies...
  pushd "%~dp0web-ui"
  call npm ci
  if errorlevel 1 (popd & goto :node_modules_error)
  popd
)

if not exist "%~dp0logs" mkdir "%~dp0logs"
echo Starting PassoFlow API server...
start "PassoFlow API" /b cmd /c ""%PYTHON_BIN%" "%~dp0src\api_server.py" > "%~dp0logs\api_server_windows.log" 2>&1"
echo Starting PassoFlow web UI...
start "PassoFlow UI" /b cmd /c "cd /d ""%~dp0web-ui"" ^&^& npm run dev -- --host 127.0.0.1 --strictPort"
set "APP_URL=http://127.0.0.1:5173"
goto :wait_for_app

:start_packaged
if not exist "%~dp0logs" mkdir "%~dp0logs"
echo Starting PassoFlow packaged server...
start "PassoFlow API" /b cmd /c ""%PYTHON_BIN%" "%~dp0src\api_server.py" > "%~dp0logs\api_server_windows.log" 2>&1"
set "APP_URL=http://127.0.0.1:8000"

:wait_for_app
echo Waiting for PassoFlow to become ready...
for /l %%N in (1,1,30) do (
  powershell -NoProfile -ExecutionPolicy Bypass -Command "try { Invoke-WebRequest -UseBasicParsing -TimeoutSec 1 '%APP_URL%' | Out-Null; exit 0 } catch { exit 1 }" >nul 2>&1
  if not errorlevel 1 goto :ready
  timeout /t 1 /nobreak >nul
)
echo PassoFlow did not start within 30 seconds.
echo Check logs\api_server_windows.log and the UI terminal output.
pause
exit /b 1

:ready
echo Opening %APP_URL%
start "" "%APP_URL%"
echo.
echo PassoFlow is ready. Keep this window open while using the editor.
echo Close this window to stop the launcher.
pause
exit /b 0

:python_error
echo Python 3.10 or later was not found.
echo Install Python, then run: pip install -r requirements.txt
pause
exit /b 1

:dependency_error
echo PassoFlow Python dependencies are missing.
echo Run: "%PYTHON_BIN%" -m pip install -r requirements.txt
pause
exit /b 1

:node_error
echo Node.js and npm were not found. Install Node.js to run the source checkout.
pause
exit /b 1

:node_modules_error
echo Web UI dependencies could not be installed.
echo Run manually: cd web-ui ^&^& npm ci
pause
exit /b 1
