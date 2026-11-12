# clickr

Fast macro clicker. Tauri 2: Rust engine + small web UI. Made by 6a5m — discord @6a5m.

## Get the .exe (no installs needed)
1. Make a free GitHub account, create a **new private repository**.
2. Upload everything in this folder to it (drag & drop in the browser works).
3. Open the **Actions** tab -> **build-windows** -> wait ~5 min -> download the **clickr-windows** artifact -> unzip -> run `clickr.exe`.

(Or install Rust + VS Build Tools and double-click `build-windows.bat`.)

## Use it
- Pick a **trigger key** (default F6) and **hold**/**toggle** mode.
- Add the **keys / buttons** to press. Trigger key can't also be an output key.
- Set **target cps**. Settings save automatically.
- Windows 10 1803+ recommended (high-resolution timer). If your game runs as admin, run clickr as admin too.

## How it stays fast
- Absolute schedule: sends exactly `floor(t*cps)+1` clicks by time t, so lateness never lowers the average rate.
- Press+release for many clicks go out in one `SendInput` call; wakes at most once per 1 ms, so 5000 cps = ~1000 syscalls/s instead of 5000+.
- High-res waitable timer for the wait, a tiny spin only for the last ~20-250 µs; clicker thread is HIGHEST priority (not time-critical), so it never starves the game.
- Window stats pause when the window is hidden.

Layout: `ui/` (HTML/CSS/JS), `src-tauri/src/` (`engine.rs`, `pacer.rs`, `keys.rs`, `win.rs`).
