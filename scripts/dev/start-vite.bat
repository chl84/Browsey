@echo off
setlocal
pushd "%~dp0\..\..\frontend"
npm run dev -- --host --port 5173 --strictPort --clearScreen false
popd
