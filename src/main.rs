#![windows_subsystem = "windows"]

mod devices;
mod state;
mod tray;

use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};

use devices::attack_shark::start_attack_shark_worker;
use state::AppState;
use tray::{run_tray_loop, WM_REFRESH_TRAY};

fn main() {
    let state = Arc::new(AppState::new());

    // Start background worker for Attack Shark R1
    let on_update = Box::new(|| unsafe {
        let class_name: Vec<u16> = std::ffi::OsStr::new("MefBatteryTrayClass")
            .encode_wide()
            .chain(Some(0))
            .collect();
        let hwnd = FindWindowW(class_name.as_ptr(), std::ptr::null());
        if hwnd != 0 {
            PostMessageW(hwnd, WM_REFRESH_TRAY, 0, 0);
        }
    });

    start_attack_shark_worker(state.clone(), on_update);

    // Run the native Windows system tray loop on the main thread
    unsafe {
        run_tray_loop(state);
    }
}
