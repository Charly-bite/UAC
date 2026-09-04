//! Sound module for playing the authentic Windows UAC prompt sound.

#[link(name = "winmm")]
unsafe extern "system" {
    fn PlaySoundW(psz_sound: *const u16, hmod: usize, fdw_sound: u32) -> i32;
}

const SND_ASYNC: u32 = 0x0001;
const SND_NODEFAULT: u32 = 0x0002;
const SND_ALIAS: u32 = 0x00010000;
const SND_FILENAME: u32 = 0x00020000;
const SND_SYSTEM: u32 = 0x00200000;

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Plays the authentic Windows UAC elevation prompt sound asynchronously.
pub fn play_uac_sound() {
    unsafe {
        // Try playing system event sound alias first
        let alias = encode_wide("WindowsUAC");
        let res = PlaySoundW(
            alias.as_ptr(),
            0,
            SND_ASYNC | SND_ALIAS | SND_NODEFAULT | SND_SYSTEM,
        );

        if res == 0 {
            // Fallback: direct standard file path on Windows 10/11
            let path = encode_wide(r"C:\Windows\Media\Windows User Account Control.wav");
            let _ = PlaySoundW(
                path.as_ptr(),
                0,
                SND_ASYNC | SND_FILENAME | SND_NODEFAULT,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_uac_sound() {
        // Must execute without panic or crash
        play_uac_sound();
    }
}
