use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Disconnected,
    Active,
    Charging,
    FullyCharged,
}

impl std::fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceStatus::Disconnected => write!(f, "Отключено"),
            DeviceStatus::Active => write!(f, "Работает от батареи"),
            DeviceStatus::Charging => write!(f, "Заряжается"),
            DeviceStatus::FullyCharged => write!(f, "Полностью заряжено"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub battery_pct: u8,
    pub status: DeviceStatus,
    pub is_connected: bool,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            name: "Attack Shark R1".to_string(),
            battery_pct: 0,
            status: DeviceStatus::Disconnected,
            is_connected: false,
        }
    }
}

pub struct AppState {
    pub device: RwLock<DeviceInfo>,
    pub running: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            device: RwLock::new(DeviceInfo::default()),
            running: AtomicBool::new(true),
        }
    }

    pub fn update_device(&self, pct: u8, status: DeviceStatus, connected: bool) -> bool {
        let mut dev = self.device.write().unwrap();
        let changed = dev.battery_pct != pct || dev.status != status || dev.is_connected != connected;
        dev.battery_pct = pct.min(100);
        dev.status = status;
        dev.is_connected = connected;
        changed
    }

    pub fn get_device_info(&self) -> DeviceInfo {
        self.device.read().unwrap().clone()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
