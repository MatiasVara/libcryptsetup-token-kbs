// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{anyhow, Result};
use log::debug;
use serde_json::Value;
use toml;

const TRUSTEE_KBS_URL_KEY: &str = "trustee.kbs.url";
const TRUSTEE_KBS_RESOURCE_KEY: &str = "trustee.kbs.resource";

#[derive(Debug)]
pub struct InitData {
    data: Vec<u8>,
    kbs_url: String,
    kbs_resource: String,
}

pub fn new_initdata_from_json(json: &str) -> Result<InitData> {
    let v: Value = serde_json::from_str(json)?;
    let kbs_url = v
        .get("trustee.kbs.url")
        .and_then(|u| u.as_str())
        .ok_or_else(|| anyhow!("Missing or invalid 'trustee.kbs.url' in token JSON"))?;
    let kbs_resource = v
        .get("trustee.kbs.resource")
        .and_then(|r| r.as_str())
        .ok_or_else(|| anyhow!("Missing or invalid 'trustee.kbs.resource' in token JSON"))?;
    Ok(InitData {
        data: Vec::new(),
        kbs_url: kbs_url.to_string(),
        kbs_resource: kbs_resource.to_string(),
    })
}

pub fn new_initdata(data: Vec<u8>) -> Result<InitData> {
    let doc = String::from_utf8_lossy(&data).into_owned();
    debug!("Loading initdata.toml file <<{}>>", doc);
    let toml = doc.parse::<toml::Table>()?;
    let (kbs_url, kbs_resource) = validate(&toml)?;
    Ok(InitData {
        data,
        kbs_url,
        kbs_resource,
    })
}

fn validate(toml: &toml::Table) -> Result<(String, String)> {
    if !toml.contains_key("algorithm") {
        return Err(anyhow!("Missing 'algorithm' key in initdata TOML header"));
    }
    if !toml.contains_key("version") {
        return Err(anyhow!("Missing 'version' key in initdata TOML header"));
    }
    if toml["version"].as_str() != Some("0.1.0") {
        return Err(anyhow!(
            "Invalid 'version' in initdata TOML {:#?} header, expected '0.1.0'",
            toml["version"].as_str()
        ));
    }
    if !toml.contains_key("data") {
        return Err(anyhow!("Missing 'data' key in initdata TOML"));
    }
    match toml["data"].as_table() {
        Some(data) => {
            let url = data[TRUSTEE_KBS_URL_KEY]
                .as_str()
                .ok_or_else(|| anyhow!("Missing or invalid {} in initdata", TRUSTEE_KBS_URL_KEY))?;
            let res = data[TRUSTEE_KBS_RESOURCE_KEY].as_str().ok_or_else(|| {
                anyhow!(
                    "Missing or invalid {} in initdata",
                    TRUSTEE_KBS_RESOURCE_KEY
                )
            })?;
            Ok((url.to_string(), res.to_string()))
        }
        None => Err(anyhow!(
            "Invalid 'data' key in initdata TOML; value must be a table"
        )),
    }
}

impl InitData {
    pub fn as_str(&self) -> String {
        String::from_utf8_lossy(&self.data).into_owned()
    }
    pub fn kbs_url(&self) -> &str {
        &self.kbs_url
    }
    pub fn kbs_resource(&self) -> &str {
        &self.kbs_resource
    }
}
