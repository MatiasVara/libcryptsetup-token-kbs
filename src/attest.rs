// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{anyhow, Result};
use log::{debug, info};
use std::path::Path;
use std::process::Command;

use crate::initdata::InitData;

const KBS_CA_CERT_PATH: &str = "/etc/trustee-attester/kbs-ca.pem";

pub fn get_resource(initdata: &InitData) -> Result<Vec<u8>> {
    let mut cmd = Command::new("trustee-attester");
    cmd.arg("--url").arg(initdata.kbs_url());

    if Path::new(KBS_CA_CERT_PATH).exists() {
        info!("Using KBS CA certificate from {}", KBS_CA_CERT_PATH);
        cmd.arg("--cert-file").arg(KBS_CA_CERT_PATH);
    }

    cmd.arg("get-resource")
        .arg("--path")
        .arg(initdata.kbs_resource());

    // if initdata is passed, mrconfig/hostData will be
    // included in the attestation token
    if !initdata.as_str().is_empty() {
        cmd.arg("--initdata").arg(initdata.as_str());
    }

    debug!("Running {:?}", cmd);
    let output = cmd
        .output()
        .map_err(|e| anyhow!("Unable to spawn trustee-attester: {}", e))?;

    debug!("trustee-attester exit status: {:?}", output.status);
    if !output.stderr.is_empty() {
        debug!(
            "trustee-attester stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(anyhow!(
            "trustee-attester failed for {} resource {}: {}",
            initdata.kbs_url(),
            initdata.kbs_resource(),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
