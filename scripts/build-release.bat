@echo off
setlocal EnableExtensions EnableDelayedExpansion

:: Build Windows release bundle (NSIS) in one step. Frontend is built by Tauri's beforeBuildCommand.
set ROOT=%~dp0..

pushd "%ROOT%" || exit /b 1

echo Building Tauri NSIS bundle...
cargo tauri build --config "%ROOT%\tauri.conf.json" --bundles nsis %*
if errorlevel 1 (
  echo Tauri build failed
  popd
  exit /b 1
)

echo Done. Installer at target\release\bundle\nsis
popd
