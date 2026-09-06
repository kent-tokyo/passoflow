@echo off
REM Run src/test.py from the project root, regardless of the caller's current directory.

cd /d "%~dp0"
python src\test.py

pause
