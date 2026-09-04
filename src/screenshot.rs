//! Screenshot module using native Win32 GDI to capture and dim the desktop.

use std::mem::size_of;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct CapturedScreen {
    pub hbitmap: HBITMAP,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Drop for CapturedScreen {
    fn drop(&mut self) {
        if !self.hbitmap.is_invalid() {
            unsafe {
                let _ = DeleteObject(self.hbitmap.into());
            }
        }
    }
}

/// Captures the full virtual screen (multi-monitor supported) and applies a dimming effect.
pub fn capture_and_dim_desktop() -> Option<CapturedScreen> {
    unsafe {
        let mut x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let mut y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let mut width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let mut height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        if width <= 0 || height <= 0 {
            x = 0;
            y = 0;
            width = GetSystemMetrics(SM_CXSCREEN);
            height = GetSystemMetrics(SM_CYSCREEN);
        }

        let hdc_screen = GetDC(None);
        if hdc_screen.is_invalid() {
            return None;
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            let _ = ReleaseDC(None, hdc_screen);
            return None;
        }

        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbitmap.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(None, hdc_screen);
            return None;
        }

        let old_bmp = SelectObject(hdc_mem, hbitmap.into());
        let blt_res = BitBlt(
            hdc_mem,
            0,
            0,
            width,
            height,
            Some(hdc_screen),
            x,
            y,
            SRCCOPY,
        );

        if blt_res.is_err() {
            let _ = SelectObject(hdc_mem, old_bmp);
            let _ = DeleteObject(hbitmap.into());
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(None, hdc_screen);
            return None;
        }

        // Prepare BITMAPINFO for 32bpp top-down DIB
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let pixel_count = (width * height) as usize;
        let mut buffer: Vec<u8> = vec![0u8; pixel_count * 4];

        // Read pixels
        let lines_read = GetDIBits(
            hdc_screen,
            hbitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        if lines_read > 0 {
            // Apply authentic Windows Secure Desktop dimming (~36% brightness)
            for pixel in buffer.chunks_exact_mut(4) {
                // pixel: [B, G, R, A]
                pixel[0] = ((pixel[0] as u32 * 36) / 100) as u8;
                pixel[1] = ((pixel[1] as u32 * 36) / 100) as u8;
                pixel[2] = ((pixel[2] as u32 * 36) / 100) as u8;
            }

            // Write back dimmed pixels to HBITMAP
            SetDIBits(
                Some(hdc_screen),
                hbitmap,
                0,
                height as u32,
                buffer.as_ptr() as *const _,
                &bmi,
                DIB_RGB_COLORS,
            );
        }

        // Cleanup DCs
        let _ = SelectObject(hdc_mem, old_bmp);
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(None, hdc_screen);

        Some(CapturedScreen {
            hbitmap,
            x,
            y,
            width,
            height,
        })
    }
}
