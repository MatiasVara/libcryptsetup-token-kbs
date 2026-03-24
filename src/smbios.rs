// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{anyhow, Result};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use log::debug;
use smbioslib;

use crate::initdata;

pub fn load_from_oem_strings() -> Result<initdata::InitData> {
    debug!("Attempting to load SMBIOS tables");
    let smbios = smbioslib::table_load_from_device()?;

    debug!(
        "Checking if any SMBIOS OEM Strings table exists \
         and contains a valid base64 encoded initdata TOML document"
    );
    for table in smbios.collect::<smbioslib::SMBiosOemStrings>() {
        for string in table.oem_strings() {
            match string.to_utf8_lossy() {
                Some(base64) => match BASE64_STANDARD.decode(base64.clone()) {
                    Ok(bytes) => match initdata::new_initdata(bytes) {
                        Ok(initdata) => {
                            return Ok(initdata);
                        }
                        Err(e) => debug!("Invalid initdata TOML: {}", e),
                    },
                    Err(e) => debug!(
                        "Invalid initdata document {}, cannot decode base64 {}",
                        base64, e
                    ),
                },
                None => debug!("Invalid OEM string"),
            }
        }
    }

    Err(anyhow!("No valid OEM strings data"))
}
