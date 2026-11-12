@echo off
REM Local build on Windows. Needs Rust (rustup.rs) + "Desktop development with C++" from Visual Studio Build Tools.
cd /d "%~dp0src-tauri"
cargo build --release --features custom-protocol
echo.
echo Done: %~dp0src-tauri\target\release\clickr.exe
pause
