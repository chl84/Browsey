@echo off
setlocal
pushd "%~dp0\..\..\frontend" || exit /b 1
npm run dev -- --host --port 5173 --strictPort --clearScreen false
set "CODE=%errorlevel%"
popd
exit /b %CODE%
