@echo off
setlocal
pushd "%~dp0\..\..\frontend" || exit /b 1
npm run build
set "CODE=%errorlevel%"
popd
exit /b %CODE%
