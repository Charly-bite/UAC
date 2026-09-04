//! Low-level keyboard hook to block system hotkeys and implement Ctrl+Q secret exit.
//!
//! Blocks: Alt+Tab, Alt+F4, Alt+Esc, Ctrl+Esc, Win key (L/R)
//! Secret Exit: Ctrl+Q

use std::sync::atomic::{AtomicIsize, Ordering};
use std::os::windows::process::CommandExt;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// Stored hook handle for cleanup
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);

// Virtual key code for 'Q' (not a named constant in the crate)
const VK_Q_CODE: u32 = 0x51;

// KBDLLHOOKSTRUCT flags
const LLKHF_ALTDOWN: u32 = 0x20;

/// Installs the low-level keyboard hook. Must be called from the thread
/// that will run the message pump (hook events are dispatched via the message loop).
pub fn install_keyboard_hook() {
    unsafe {
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), None, 0) {
            Ok(hook) => {
                HOOK_HANDLE.store(hook.0 as isize, Ordering::SeqCst);
            }
            Err(_) => {
                // Silently continue — hooks are non-critical
            }
        }
    }
}

/// Removes the keyboard hook. Safe to call even if the hook was never installed.
pub fn uninstall_keyboard_hook() {
    let raw = HOOK_HANDLE.swap(0, Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(raw as *mut _));
        }
    }
}

/// Low-level keyboard hook procedure.
/// Returns LRESULT(1) to swallow the keystroke, or calls CallNextHookEx to pass it along.
unsafe extern "system" fn ll_keyboard_proc(
    n_code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if n_code as u32 == HC_ACTION {
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = kb.vkCode;
        let alt_down = (kb.flags.0 & LLKHF_ALTDOWN) != 0;

        // Check Ctrl state via GetAsyncKeyState
        let ctrl_down = (unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } & (0x8000u16 as i16)) != 0;

        // ── Secret Exit: Ctrl+Q ──────────────────────────────────────────
        if ctrl_down && vk == VK_Q_CODE {
            unsafe {
                let _ = PostQuitMessage(0);
            }
            // Allow the keypress to pass through (it will be consumed by quit)
            return unsafe {
                CallNextHookEx(
                    None,
                    n_code,
                    wparam,
                    lparam,
                )
            };
        }

        // ── Block: Windows keys (L/R) ────────────────────────────────────
        if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
            return LRESULT(1);
        }

        // ── Block: Alt+Tab ───────────────────────────────────────────────
        if alt_down && vk == VK_TAB.0 as u32 {
            return LRESULT(1);
        }

        // ── Block: Alt+F4 ────────────────────────────────────────────────
        if alt_down && vk == VK_F4.0 as u32 {
            return LRESULT(1);
        }

        // ── Block: Alt+Esc ───────────────────────────────────────────────
        if alt_down && vk == VK_ESCAPE.0 as u32 {
            return LRESULT(1);
        }

        // ── Block: Ctrl+Esc (Start menu) ─────────────────────────────────
        if ctrl_down && vk == VK_ESCAPE.0 as u32 {
            return LRESULT(1);
        }

        // ── Block: Ctrl+Shift+Esc (Task Manager) ─────────────────────────
        let shift_down = (unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) } & (0x8000u16 as i16)) != 0;
        if ctrl_down && shift_down && vk == VK_ESCAPE.0 as u32 {
            return LRESULT(1);
        }

        // ── Block: Ctrl+Alt+Delete cannot be intercepted via hooks ───────
        // (handled by the Windows kernel / Winlogon — no user-mode hook can block it)
        // However, we disable Task Manager via registry so even if the user
        // reaches the Ctrl+Alt+Del screen, they cannot kill this process.
    }

    unsafe {
        CallNextHookEx(
            None,
            n_code,
            wparam,
            lparam,
        )
    }
}

/// Disables Task Manager via the Windows registry (HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System).
/// This prevents the user from killing the process via Ctrl+Alt+Supr → Task Manager.
pub fn disable_task_manager() {
    use std::process::Command;
    let _ = Command::new("reg")
        .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System",
               "/v", "DisableTaskMgr", "/t", "REG_DWORD", "/d", "1", "/f"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW — hide the console flash
        .output();
}

/// Re-enables Task Manager by removing the registry restriction.
pub fn enable_task_manager() {
    use std::process::Command;
    let _ = Command::new("reg")
        .args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System",
               "/v", "DisableTaskMgr", "/f"])
        .creation_flags(0x08000000)
        .output();
}
