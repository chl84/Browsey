@echo off
setlocal

rem Run Tauri dev without spawning the frontend dev server (Vite started by beforeDevCommand).
cd /d %~dp0\..\.. || exit /b 1
set "PATH=%USERPROFILE%\.cargo\bin;C:\Program Files\nodejs;%PATH%"
set "TAURI_CLI_WATCHER_IGNORE_FILENAME=.taurignore"

cargo tauri dev --no-dev-server %*
exit /b %errorlevel%
