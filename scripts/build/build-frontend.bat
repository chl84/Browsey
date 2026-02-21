@echo off
setlocal
pushd "%~dp0\..\..\frontend"
npm run build
popd
