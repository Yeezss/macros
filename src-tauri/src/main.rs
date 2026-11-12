#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod keys;
mod pacer;
#[cfg(windows)]
#[path = "win.rs"]
mod sys;
#[cfg(not(windows))]
#[path = "stub.rs"]
mod sys;

use engine::{Config, Notifier, Shared};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

struct UiNotifier(AppHandle);

#[derive(Serialize, Clone)]
struct Stats {
    cps: f64,
    cpu: f64,
}

impl Notifier for UiNotifier {
    fn running(&self, on: bool) {
        let _ = self.0.emit("clickr://running", on);
    }
    fn stats(&self, cps: f64, cpu: f64) {
        let _ = self.0.emit("clickr://stats", Stats { cps, cpu });
    }
}

#[tauri::command]
fn update_config(config: Config, state: State<'_, Arc<Shared>>) {
    state.set_config(config);
}

fn main() {
    let shared = Arc::new(Shared::default());
    tauri::Builder::default()
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![update_config])
        .setup(move |app| {
            engine::spawn(shared.clone(), Arc::new(UiNotifier(app.handle().clone())));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running clickr");
}
