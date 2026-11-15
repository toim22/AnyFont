@echo off
rem ===========================================================================
rem  AnyFont - build and package (Rust + MSVC x64)
rem
rem    build.bat             Release + product package in dist\
rem    build.bat test        the workspace tests (cargo test --workspace)
rem    build.bat debug       Debug (target\debug)
rem    build.bat clean       deletes target\ and dist\
rem
rem  Release output:
rem    dist\AnyFont\                       program folder (anyfont.exe, licence and notices)
rem    dist\AnyFont-<version>-win-x64.zip  portable package for distribution
rem    dist\AnyFont-<version>-setup.exe    installer (only if Inno Setup 6.7+ is installed:
rem                                        winget install JRSoftware.InnoSetup)
rem    dist\symbols\                       PDBs (crash debugging; do not distribute)
rem
rem  The version is the one in the root Cargo.toml ([workspace.package] version).
rem  Needs Rust (https://rustup.rs, the x86_64-pc-windows-msvc toolchain) and the Visual Studio C++ build tools
rem  (the linker, rc.exe and the Windows SDK).
rem ===========================================================================
setlocal
set "ROOT=%~dp0"
cd /d "%ROOT%"

if /i "%~1"=="clean" (
  if exist target rmdir /s /q target
  if exist dist rmdir /s /q dist
  echo Clean.
  exit /b 0
)

where cargo >nul 2>nul || (echo [error] cargo not found. Install Rust from https://rustup.rs & exit /b 1)

if /i "%~1"=="debug" (
  cargo build --workspace || goto :fail
  echo.
  echo Debug build OK: target\debug\anyfont.exe
  exit /b 0
)
if /i "%~1"=="test" (
  cargo test --workspace || goto :fail
  exit /b 0
)
if not "%~1"=="" (
  echo [error] unknown option "%~1". Use: build.bat [test^|debug^|clean]
  exit /b 1
)

rem ---- the version (root Cargo.toml, [workspace.package]: the first line that starts with "version")
set "VER="
for /f "tokens=2 delims==" %%v in ('findstr /r /b /c:"version *=" Cargo.toml') do if not defined VER set "VER=%%~v"
set "VER=%VER: =%"
set "VER=%VER:"=%"
if not defined VER (echo [error] no version in Cargo.toml & exit /b 1)

rem ---- brand assets (the icon and the wizard pictures) if they were never generated
if not exist crates\anyfont\assets\anyfont.ico set "NEEDBRAND=1"
if not exist installer\WizardLarge-100.png set "NEEDBRAND=1"
if defined NEEDBRAND (
  echo Generating the brand assets: cargo run -p xtask -- brand
  cargo run -p xtask -- brand || goto :fail
)

rem ---- the open-source components and their licences (THIRD-PARTY-NOTICES.txt), before the build
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\licenses.ps1 || goto :fail

rem only the program (xtask and the tests are not part of the package)
cargo build --release -p anyfont || goto :fail

rem ---- program folder and symbols
set "BIN=target\release"
if exist dist\AnyFont rmdir /s /q dist\AnyFont
if exist dist\symbols rmdir /s /q dist\symbols
mkdir dist\AnyFont\legal dist\symbols || goto :fail
copy /y "%BIN%\anyfont.exe" dist\AnyFont\ >nul || goto :fail
if exist "%BIN%\anyfont.pdb" copy /y "%BIN%\anyfont.pdb" dist\symbols\ >nul
for %%f in (README.md CHANGELOG.md LICENSE) do copy /y %%f dist\AnyFont\ >nul || goto :fail
copy /y THIRD-PARTY-NOTICES.txt dist\AnyFont\legal\ >nul || goto :fail

rem ---- portable package (one top folder)
set "PKG=AnyFont-%VER%-win-x64"
if exist dist\%PKG%.zip del /q dist\%PKG%.zip
if exist dist\_zip rmdir /s /q dist\_zip
xcopy /e /i /q /y dist\AnyFont dist\_zip\%PKG% >nul || goto :fail
powershell -NoProfile -Command "Compress-Archive -Path 'dist\_zip\%PKG%' -DestinationPath 'dist\%PKG%.zip' -CompressionLevel Optimal" || goto :fail
rmdir /s /q dist\_zip

rem ---- installer (Inno Setup 6.7+) with the same version
if exist dist\AnyFont-*-setup.exe del /q dist\AnyFont-*-setup.exe
set "ISCC="
for %%p in ("%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" "%ProgramFiles%\Inno Setup 6\ISCC.exe") do if not defined ISCC if exist "%%~p" set "ISCC=%%~p"
if not defined ISCC for /f "delims=" %%p in ('where iscc 2^>nul') do if not defined ISCC set "ISCC=%%p"
if not defined ISCC goto :noinno
"%ISCC%" /Q /DAppVersion=%VER% installer\AnyFont.iss || goto :fail
:noinno

echo.
echo Build OK (version %VER%)
echo   Program:    dist\AnyFont\anyfont.exe
for %%z in (dist\AnyFont-*-win-x64.zip) do echo   Package:    %%z
for %%s in (dist\AnyFont-*-setup.exe) do echo   Installer:  %%s
if not defined ISCC echo   Installer:  Inno Setup 6 not found - install it with: winget install JRSoftware.InnoSetup
exit /b 0

:fail
echo.
echo [error] build failed
exit /b 1
