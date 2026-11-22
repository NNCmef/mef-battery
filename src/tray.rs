use std::sync::Arc;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC, DeleteObject, FillRect,
    SelectObject, SetBkMode, SetTextColor, TextOutA, TRANSPARENT, HBITMAP, HDC, RECT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyMenu, DispatchMessageW, GetCursorPos, GetMessageW, PostQuitMessage,
    RegisterClassW, SetForegroundWindow, TrackPopupMenuEx, TranslateMessage, HICON, HMENU,
    ICONINFO, MF_DISABLED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, TPM_BOTTOMALIGN,
    TPM_RIGHTALIGN, TPM_RETURNCMD, WM_APP, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

use crate::state::{AppState, DeviceStatus};

const WM_TRAYICON: u32 = WM_APP + 1;
pub const WM_REFRESH_TRAY: u32 = WM_APP + 2;

const CMD_EXIT: u32 = 1001;

pub struct TrayManager {
    hwnd: HWND,
    nid: NOTIFYICONDATAW,
    current_icon: HICON,
    state: Arc<AppState>,
}

static mut GLOBAL_STATE: Option<Arc<AppState>> = None;

pub unsafe fn run_tray_loop(state: Arc<AppState>) {
    GLOBAL_STATE = Some(state.clone());

    let h_instance = GetModuleHandleW(std::ptr::null());
    let class_name = to_wide_chars("MefBatteryTrayClass");

    let wnd_class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: h_instance,
        hIcon: 0,
        hCursor: 0,
        hbrBackground: 0,
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };

    RegisterClassW(&wnd_class);

    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        to_wide_chars("MefBatteryTrayWindow").as_ptr(),
        WS_OVERLAPPEDWINDOW,
        0,
        0,
        0,
        0,
        0,
        0,
        h_instance,
        std::ptr::null(),
    );

    let initial_icon = create_battery_icon(0, DeviceStatus::Disconnected);
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = initial_icon;

    set_tip(&mut nid, "mef battery: Поиск устройств...");
    Shell_NotifyIconW(NIM_ADD, &nid);

    // Main Windows message loop
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, 0, 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    // Cleanup tray icon on quit
    Shell_NotifyIconW(NIM_DELETE, &nid);
    if initial_icon != 0 {
        DestroyIcon(initial_icon);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = (lparam & 0xFFFF) as u32;
            if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                show_context_menu(hwnd);
            }
            0
        }
        WM_REFRESH_TRAY => {
            if let Some(ref state) = GLOBAL_STATE {
                update_tray_icon(hwnd, state);
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn update_tray_icon(hwnd: HWND, state: &AppState) {
    let dev = state.get_device_info();

    let new_icon = create_battery_icon(dev.battery_pct, dev.status);

    let tip = if dev.is_connected {
        format!("{}: {}% ({})", dev.name, dev.battery_pct, dev.status)
    } else {
        format!("{}: Отключено", dev.name)
    };

    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_ICON | NIF_TIP;
    nid.hIcon = new_icon;
    set_tip(&mut nid, &tip);

    Shell_NotifyIconW(NIM_MODIFY, &nid);
}

unsafe fn show_context_menu(hwnd: HWND) {
    let mut pt: POINT = std::mem::zeroed();
    GetCursorPos(&mut pt);

    let menu = CreatePopupMenu();
    if menu == 0 {
        return;
    }

    let dev = if let Some(ref state) = GLOBAL_STATE {
        state.get_device_info()
    } else {
        Default::default()
    };

    // Header title
    let title = to_wide_chars("⚡ mef battery v0.1.0");
    AppendMenuW(menu, MF_STRING | MF_GRAYED | MF_DISABLED, 0, title.as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

    // Device status row
    let status_str = if dev.is_connected {
        format!("{}: {}% ({})", dev.name, dev.battery_pct, dev.status)
    } else {
        format!("{}: Не подключено", dev.name)
    };
    let device_item = to_wide_chars(&status_str);
    AppendMenuW(menu, MF_STRING | MF_GRAYED | MF_DISABLED, 0, device_item.as_ptr());

    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

    // Exit option
    let exit_text = to_wide_chars("Выход");
    AppendMenuW(menu, MF_STRING, CMD_EXIT as usize, exit_text.as_ptr());

    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenuEx(
        menu,
        TPM_RETURNCMD | TPM_RIGHTALIGN | TPM_BOTTOMALIGN,
        pt.x,
        pt.y,
        hwnd,
        std::ptr::null(),
    );

    if cmd as u32 == CMD_EXIT {
        if let Some(ref state) = GLOBAL_STATE {
            state.stop();
        }
        PostQuitMessage(0);
    }

    DestroyMenu(menu);
}

unsafe fn create_battery_icon(pct: u8, status: DeviceStatus) -> HICON {
    let width = 16;
    let height = 16;

    let hdc_screen = CreateCompatibleDC(0);

    // 1-bit mask bitmap
    let hbm_mask = CreateBitmap(width, height, 1, 1, std::ptr::null());
    // 32-bit color bitmap
    let hbm_color = CreateBitmap(width, height, 1, 32, std::ptr::null());

    let old_bmp = SelectObject(hdc_screen, hbm_color as _);

    // Background: dark transparent canvas
    let bg_brush = CreateSolidBrush(0x00000000);
    let rect_all = RECT { left: 0, top: 0, right: width, bottom: height };
    FillRect(hdc_screen, &rect_all, bg_brush);
    DeleteObject(bg_brush as _);

    // Battery outline (left: 1, top: 3, right: 13, bottom: 13)
    let border_color = 0x00FFFFFF; // White outline
    let border_brush = CreateSolidBrush(border_color);
    let body_rect = RECT { left: 1, top: 3, right: 13, bottom: 13 };
    FillRect(hdc_screen, &body_rect, border_brush);
    DeleteObject(border_brush as _);

    // Battery tip (nipple on right: left 13, top 6, right 15, bottom 10)
    let tip_brush = CreateSolidBrush(border_color);
    let tip_rect = RECT { left: 13, top: 6, right: 15, bottom: 10 };
    FillRect(hdc_screen, &tip_rect, tip_brush);
    DeleteObject(tip_brush as _);

    // Clear inner cavity
    let inner_brush = CreateSolidBrush(0x00202020);
    let inner_rect = RECT { left: 2, top: 4, right: 12, bottom: 12 };
    FillRect(hdc_screen, &inner_rect, inner_brush);
    DeleteObject(inner_brush as _);

    // Fill battery gauge based on percentage and status
    let fill_color = match status {
        DeviceStatus::Charging => 0x00FFD700,      // Amber/Gold or Cyan
        DeviceStatus::FullyCharged => 0x0000FF7F,  // Spring Green
        DeviceStatus::Disconnected => 0x00555555,  // Dim Gray
        DeviceStatus::Active => {
            if pct > 40 {
                0x0000E676 // Bright green
            } else if pct > 20 {
                0x0000D7FF // Yellow
            } else {
                0x003333FF // Red
            }
        }
    };

    if status != DeviceStatus::Disconnected {
        let max_width = 10;
        let fill_w = ((pct as f32 / 100.0) * max_width as f32).round().max(1.0) as i32;
        let fill_brush = CreateSolidBrush(fill_color);
        let gauge_rect = RECT { left: 2, top: 4, right: 2 + fill_w, bottom: 12 };
        FillRect(hdc_screen, &gauge_rect, fill_brush);
        DeleteObject(fill_brush as _);
    }

    SelectObject(hdc_screen, old_bmp);
    DeleteDC(hdc_screen);

    let mut ii = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };

    let icon = CreateIconIndirect(&mut ii);

    DeleteObject(hbm_mask as _);
    DeleteObject(hbm_color as _);

    icon
}

fn set_tip(nid: &mut NOTIFYICONDATAW, text: &str) {
    let wide = to_wide_chars(text);
    let len = wide.len().min(nid.szTip.len() - 1);
    for i in 0..len {
        nid.szTip[i] = wide[i];
    }
    nid.szTip[len] = 0;
}

fn to_wide_chars(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
