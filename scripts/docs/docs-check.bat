@echo off
setlocal EnableExtensions EnableDelayedExpansion

set ROOT=%~dp0\..\..

pushd "%ROOT%" || exit /b 1
npm --prefix "%ROOT%\docs" run check -- %*
set ERR=%ERRORLEVEL%
popd
exit /b %ERR%
