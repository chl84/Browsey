@echo off
setlocal EnableDelayedExpansion

:: Build Windows release bundle (NSIS). Cleans old bundles, builds frontend, then runs Tauri bundler.

set ROOT=%~dp0..
pushd "%ROOT%"

if exist "target\release\bundle" (
  echo Removing target\release\bundle ...
  rmdir /s /q "target\release\bundle"
)
if exist "src-tauri\target" (
  echo Removing src-tauri\target ...
  rmdir /s /q "src-tauri\target"
)

echo Building frontend...
pushd "frontend"
npm run build
if errorlevel 1 (
  echo Frontend build failed ^(npm run build^)
  popd
  exit /b 1
)
popd

echo Building Tauri NSIS bundle...
cargo tauri build --bundles nsis %*
if errorlevel 1 (
  echo Tauri build failed
  popd
  exit /b 1
)

echo Done. Installer at target\release\bundle\nsis
popd
