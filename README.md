# Windows 11 UAC Clone & Ingestion Server

A high-fidelity Windows 11 User Account Control (UAC) credential dialog recreation written in native Rust (Win32 API) paired with a lightweight Python collection listener. Developed for authorized security assessments, red team engagements, and user awareness training.

> **Disclaimer**: This software is intended strictly for authorized educational research, security testing, and penetration testing within environments where explicit permission has been granted. Unauthorized use is prohibited.

---

## 🏛️ Project Architecture

```
UAC/
├── src/
│   ├── main.rs              # WinMain entry point with windowless subsystem
│   ├── dialog.rs            # Pixel-accurate Windows 11 UAC modal dialog rendering
│   ├── overlay.rs           # Multi-monitor desktop capture & dimmed background lock
│   ├── hooks.rs             # WH_KEYBOARD_LL hook (blocks Alt+Tab, WinKey, etc.; Ctrl+Q exit)
│   ├── screenshot.rs        # Virtual desktop GDI screen grab and dimming processor
│   ├── sound.rs             # Native Windows UAC audio prompt playback
│   ├── exfil.rs             # Asynchronous HTTP POST payload transmitter
│   ├── obfuscate.rs         # XOR + Base64 endpoint obfuscator
│   └── bin/
│       └── encode_url.rs    # CLI tool to generate obfuscated endpoint constants
├── server/
│   ├── server.py            # Dual-engine Flask & http.server collection server
│   ├── requirements.txt     # Python dependencies
│   └── README.md            # Linux deployment and systemd setup guide
├── Cargo.toml               # Rust workspace & dependencies
└── .gitignore
```

---

## ✨ Features

### Client (Rust)
- **Pixel-Accurate Win11 UI**: Native Win32 GDI custom-rendered dialog matching the latest Windows 11 design system (light mode, Segoe UI Variable, accent line input focus, and authentic layout).
- **Secure Desktop Simulation**: Captures all monitors, dims the background by ~36%, and displays a topmost modal overlay preventing access to the underlying desktop.
- **Low-Level Keyboard Hook**: `WH_KEYBOARD_LL` suppresses system hotkeys (`Alt+Tab`, `Alt+F4`, `WinKey`, `Ctrl+Esc`, `Ctrl+Shift+Esc`). Includes a secret failsafe hotkey (`Ctrl + Q`) to instantly release hooks and close the dialog.
- **Obfuscated Telemetry**: Endpoint URLs are XOR-scrambled and Base64-encoded to evade trivial static string extraction.
- **Zero Heavy Runtime Dependencies**: Pure native Win32 APIs via `windows` crate + lightweight HTTP client (`ureq`).

### Server (Python)
- **Dual-Engine Ingestion**: Operates seamlessly with Flask (with web dashboard and SQLite) or falls back to standard library `http.server` if dependencies are not installed.
- **Real-Time Terminal Banners**: Displays captured domain credentials with ANSI colored formatting.
- **Web Dashboard**: Dark-mode dashboard with real-time auto-refresh and JSON/CSV export functionality.

---

## 🚀 Quick Start

### 1. Configure Ingestion Endpoint
Generate an obfuscated URL payload using the helper utility:
```bash
cargo run --bin encode_url "http://<YOUR_SERVER_IP>:8080/api/credentials"
```
Update the generated XOR key and ciphertext in `src/exfil.rs`.

### 2. Build Client Executable
Build the optimized, stripped release binary:
```bash
cargo build --release
```
The resulting executable will be located at:
```
target/release/uac.exe
```

### 3. Run the Collection Server
On your Linux collection VM or testing host:
```bash
cd server
pip install -r requirements.txt
python server.py --host 0.0.0.0 --port 8080
```

Access the web dashboard at `http://<SERVER_IP>:8080/dashboard`.

---

## 🔒 Emergency Exit
While the prompt is active, pressing **`Ctrl + Q`** will immediately unhook all keyboard listeners and terminate the process.
