// SPDX-License-Identifier: GPL-3.0-or-later
//
// Helper binary for systemd-repart Encrypt=kbs mode.
// Performs TEE attestation via KBS and writes the LUKS key to stdout.

use std::io::Write;
use std::process::ExitCode;
use zeroize::Zeroize;

fn main() -> ExitCode {
    env_logger::init();

    let initdata = match libcryptsetup_token_kbs::fetch_initdata_from_smbios() {
        Ok(initdata) => initdata,
        Err(e) => {
            eprintln!("repart-kbs-helper: {e}");
            return ExitCode::FAILURE;
        }
    };

    match libcryptsetup_token_kbs::fetch_luks_key(&initdata) {
        Ok(mut key) => {
            if let Err(e) = std::io::stdout().write_all(&key) {
                eprintln!("repart-kbs-helper: failed to write key to stdout: {e}");
                key.zeroize();
                return ExitCode::FAILURE;
            }
            key.zeroize();
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("repart-kbs-helper: {e}");
            ExitCode::FAILURE
        }
    }
}
