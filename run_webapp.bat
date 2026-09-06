@echo off
REM Start the scenario editor's API server and web UI dev server, then open the browser.

cd /d "%~dp0"

REM Keep both services in this console instead of opening one console per service.
start "passoflow API server" /b cmd /c "python src\api_server.py"
start "passoflow web UI" /b cmd /c "cd /d ""%~dp0web-ui"" && npm run dev"

timeout /t 3 /nobreak >nul
start "" "http://localhost:5173"
echo passoflow is running. Close this window to stop the local services.
pause
