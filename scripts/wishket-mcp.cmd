@echo off
setlocal EnableExtensions
REM Windows launcher for the wishket MCP server. Mirrors scripts/wishket-mcp.
if not defined HOME if defined USERPROFILE set "HOME=%USERPROFILE%"

if defined WISHKET_PROFILE (
  if "%WISHKET_PROFILE:~0,1%"=="~" (
    set "WISHKET_PROFILE=%HOME%%WISHKET_PROFILE:~1%"
  )
)

set "PLUGIN_ROOT=%~dp0.."
set "BIN=%PLUGIN_ROOT%\server\target\release\wishket-mcp.exe"
if exist "%BIN%" (
  "%BIN%" %*
  exit /b %ERRORLEVEL%
)

where wishket-mcp >nul 2>&1
if %ERRORLEVEL%==0 (
  wishket-mcp %*
  exit /b %ERRORLEVEL%
)

where cargo >nul 2>&1
if %ERRORLEVEL%==0 (
  cargo build --release --manifest-path "%PLUGIN_ROOT%\server\Cargo.toml" 1>&2
  if errorlevel 1 exit /b 1
  "%BIN%" %*
  exit /b %ERRORLEVEL%
)

echo wishket-mcp not found and no Rust toolchain. Install the prebuilt binary: 1>&2
echo   irm https://github.com/epicsagas/wishket-radar/releases/latest/download/install.ps1 ^| iex 1>&2
exit /b 1
