// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{anyhow, Result};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use log::debug;
use toml;

const LUKS_KEY_PREFIX: &str = "io.cryptsetup.key.text.";

pub fn extract_luks_key(raw_stdout: &[u8]) -> Result<Vec<u8>> {
    let base64_str = String::from_utf8_lossy(raw_stdout).into_owned();
    let doc_bytes = BASE64_STANDARD
        .decode(base64_str.trim())
        .map_err(|e| anyhow!("Invalid confdata base64: {}", e))?;
    let doc = String::from_utf8_lossy(&doc_bytes).into_owned();

    let toml = doc.parse::<toml::Table>()?;
    if toml.get("version").and_then(|v| v.as_str()) != Some("0.1.0") {
        return Err(anyhow!("Invalid or missing 'version' in confdata TOML"));
    }
    let data = toml["data"]
        .as_table()
        .ok_or_else(|| anyhow!("Missing 'data' table in confdata TOML"))?;

    for (k, v) in data.iter() {
        if k.starts_with(LUKS_KEY_PREFIX) {
            if let Some(key_str) = v.as_str() {
                debug!("Found LUKS key entry: {}", k);
                return Ok(key_str.as_bytes().to_vec());
            }
        }
    }

    Err(anyhow!(
        "No LUKS key (io.cryptsetup.key.text.*) found in confdata"
    ))
}
