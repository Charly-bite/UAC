//! Obfuscation module for endpoint URLs and strings.
//! Uses XOR encryption with Base64 encoding to prevent plaintext strings in the compiled binary.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

const XOR_KEY: &[u8] = b"UAC_SECURE_AUTH_KEY_2026_X9";

/// Obfuscated placeholder endpoint URL: "http://127.0.0.1:8080/api/credentials"
/// This can easily be updated by replacing this constant or using `obfuscate_url`.
const OBFUSCATED_ENDPOINT_B64: &str = "PTU3L2lqbGRgcnFxe2RmbnF9aWcCH1NGNndaJyQnOj0xKjQ+Ng==";

/// Obfuscates a plaintext URL for embedding into the code.
#[allow(dead_code)]
pub fn obfuscate_url(plain: &str) -> String {
    let xored: Vec<u8> = plain
        .bytes()
        .zip(XOR_KEY.iter().cycle())
        .map(|(b, k)| b ^ k)
        .collect();
    BASE64.encode(xored)
}

/// Deobfuscates the embedded endpoint URL at runtime.
pub fn get_endpoint_url() -> String {
    match BASE64.decode(OBFUSCATED_ENDPOINT_B64) {
        Ok(decoded) => {
            let decrypted: Vec<u8> = decoded
                .into_iter()
                .zip(XOR_KEY.iter().cycle())
                .map(|(b, k)| b ^ k)
                .collect();
            String::from_utf8(decrypted).unwrap_or_else(|_| "http://127.0.0.1:8080/api/credentials".into())
        }
        Err(_) => "http://127.0.0.1:8080/api/credentials".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obfuscation_roundtrip() {
        let original = "http://127.0.0.1:8080/api/credentials";
        let obf = obfuscate_url(original);
        assert_eq!(obf, OBFUSCATED_ENDPOINT_B64);
        let recovered = get_endpoint_url();
        assert_eq!(recovered, original);
    }
}
