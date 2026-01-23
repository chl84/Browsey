@echo off
setlocal

rem Run Tauri dev without spawning the frontend dev server (Vite started by beforeDevCommand).
cd /d %~dp0..
set "PATH=%USERPROFILE%\.cargo\bin;C:\Program Files\nodejs;%PATH%"
set "TAURI_CLI_WATCHER_IGNORE_FILENAME=.taurignore"
rem Avoid PDB locking issues under OneDrive by using a local target dir
set "CARGO_TARGET_DIR=.target"

cargo tauri dev --no-dev-server %*
