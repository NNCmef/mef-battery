use std::sync::Arc;
use std::thread;
use std::time::Duration;
use hidapi::{HidApi, HidDevice};

use crate::state::{AppState, DeviceStatus};

const KNOWN_DEVICES: &[(u16, u16, &str)] = &[
    (0x1D57, 0xFA60, "Attack Shark R1 (2.4G)"),
    (0x1D57, 0xFA55, "Attack Shark R1 (Wired)"),
    (0x25A7, 0xFA60, "Attack Shark R1 / CompX (2.4G)"),
    (0x25A7, 0x3710, "Attack Shark / CompX OEM"),
];

pub fn start_attack_shark_worker(state: Arc<AppState>, on_update: Box<dyn Fn() + Send + 'static>) {
    thread::spawn(move || {
        let mut api = match HidApi::new() {
            Ok(api) => api,
            Err(e) => {
                eprintln!("[mef-battery] Failed to initialize HidApi: {:?}", e);
                return;
            }
        };

        while state.is_running() {
            let mut matched_device_info = None;
            let mut device_name = "Attack Shark R1".to_string();

            // Refresh device list
            if api.refresh_devices().is_ok() {
                for dev in api.device_list() {
                    for &(vid, pid, name) in KNOWN_DEVICES {
                        if dev.vendor_id() == vid && dev.product_id() == pid {
                            // Target telemetry interface (interface 2 or usage page 0x000A / vendor pages)
                            let is_telemetry_candidate = dev.interface_number() == 2
                                || dev.usage_page() == 0x000A
                                || dev.usage_page() >= 0xFF00
                                || dev.interface_number() == 0;

                            if is_telemetry_candidate {
                                matched_device_info = Some(dev.path().to_owned());
                                device_name = name.to_string();
                                break;
                            }
                        }
                    }
                    if matched_device_info.is_some() {
                        break;
                    }
                }
            }

            let path = match matched_device_info {
                Some(p) => p,
                None => {
                    // Device not currently connected
                    if state.update_device(0, DeviceStatus::Disconnected, false) {
                        on_update();
                    }
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            };

            // Attempt to open device
            match api.open_path(&path) {
                Ok(device) => {
                    println!("[mef-battery] Connected to {}", device_name);
                    read_device_loop(&device, &state, &on_update);
                }
                Err(_err) => {
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
    });
}

fn read_device_loop(device: &HidDevice, state: &AppState, on_update: &dyn Fn()) {
    let mut buf = [0u8; 64];

    while state.is_running() {
        match device.read_timeout(&mut buf, 1500) {
            Ok(bytes_read) if bytes_read >= 4 => {
                if let Some((pct, status)) = parse_attack_shark_report(&buf[..bytes_read]) {
                    if state.update_device(pct, status, true) {
                        on_update();
                    }
                }
            }
            Ok(_) => {
                // Timeout or partial packet, check state loop
            }
            Err(_) => {
                // Device disconnected or read error
                if state.update_device(0, DeviceStatus::Disconnected, false) {
                    on_update();
                }
                break;
            }
        }
    }
}

fn parse_attack_shark_report(data: &[u8]) -> Option<(u8, DeviceStatus)> {
    // Attack Shark telemetry report format:
    // Report ID 0x03, 5 bytes: [0x03, 0x55, 0x40, state, pct]
    // Or without report ID: [0x55, 0x40, state, pct]
    let (state_byte, pct_byte) = if data.len() >= 5 && data[0] == 0x03 && data[1] == 0x55 && data[2] == 0x40 {
        (data[3], data[4])
    } else if data.len() >= 4 && data[0] == 0x55 && data[1] == 0x40 {
        (data[2], data[3])
    } else {
        return None;
    };

    let status = match state_byte {
        2 => DeviceStatus::FullyCharged,
        3 => DeviceStatus::Charging,
        _ => DeviceStatus::Active,
    };

    let pct = pct_byte.min(100);
    Some((pct, status))
}
