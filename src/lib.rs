// SPDX-License-Identifier: GPL-3.0-or-later
//
// LUKS2 external token handler for TDX attestation via KBS.
//
// Performs TDX attestation against a Key Broker Service (KBS) using
// trustee-attester and returns the LUKS passphrase directly to
// cryptsetup, without intermediate files or external services.

pub mod attest;
pub mod confdata;
pub mod ffi;
pub mod initdata;
pub mod smbios;

use anyhow::Result;
use log::debug;

/// Fetch the LUKS key by reading SMBIOS OEM strings, performing
/// attestation via trustee-attester, and extracting the key from
/// the confdata response.
pub fn fetch_luks_key(initdata: &initdata::InitData) -> Result<Vec<u8>> {
    debug!(
        "initdata: kbs_url={}, resource={}",
        initdata.kbs_url(),
        initdata.kbs_resource()
    );
    let raw_confdata = attest::get_resource(initdata)?;
    confdata::extract_luks_key(&raw_confdata)
}

pub fn fetch_initdata_from_smbios() -> Result<initdata::InitData> {
    let initdata = smbios::load_from_oem_strings()?;
    Ok(initdata)
}
