@echo off
setlocal EnableExtensions
REM Windows launcher. Same order as scripts/wishket-mcp:
REM   1. local release binary
REM   2. installed wishket-mcp.exe
REM   3. prebuilt install.ps1
REM   4. cargo build only in a git checkout after the installer failed
if not defined HOME if defined USERPROFILE set "HOME=%USERPROFILE%"
set "PATH=%HOME%\.cargo\bin;%HOME%\.local\bin;%PATH%"

if defined WISHKET_PROFILE (
  if "%WISHKET_PROFILE:~0,1%"=="~" (
    set "WISHKET_PROFILE=%HOME%%WISHKET_PROFILE:~1%"
  )
)

set "PLUGIN_ROOT=%~dp0.."
set "BIN=%PLUGIN_ROOT%\server\target\release\wishket-mcp.exe"
set "INSTALLED=%HOME%\.local\bin\wishket-mcp.exe"
set "WRAPPER=%~f0"

if exist "%BIN%" (
  "%BIN%" %*
  exit /b %ERRORLEVEL%
)

if exist "%INSTALLED%" (
  "%INSTALLED%" %*
  exit /b %ERRORLEVEL%
)

for /f "delims=" %%i in ('where wishket-mcp 2^>nul') do (
  if /I not "%%~fi"=="%WRAPPER%" if /I not "%%~nxi"=="wishket-mcp.cmd" (
    "%%i" %*
    exit /b %ERRORLEVEL%
  )
)

if exist "%PLUGIN_ROOT%\install.ps1" (
  echo wishket-mcp: installing prebuilt binary... 1>&2
  powershell -NoProfile -ExecutionPolicy Bypass -File "%PLUGIN_ROOT%\install.ps1" 1>&2
  if exist "%INSTALLED%" (
    "%INSTALLED%" %*
    exit /b %ERRORLEVEL%
  )
)

if exist "%PLUGIN_ROOT%\.git" (
  where cargo >nul 2>&1
  if %ERRORLEVEL%==0 (
    echo wishket-mcp: prebuilt install unavailable, building from source... 1>&2
    cargo build --release --manifest-path "%PLUGIN_ROOT%\server\Cargo.toml" 1>&2
    if errorlevel 1 exit /b 1
    "%BIN%" %*
    exit /b %ERRORLEVEL%
  )
)

echo wishket-mcp not found. Install the prebuilt binary: 1>&2
echo   irm https://github.com/epicsagas/wishket-radar/releases/latest/download/install.ps1 ^| iex 1>&2
exit /b 1
