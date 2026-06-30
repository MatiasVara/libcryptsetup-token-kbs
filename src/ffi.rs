// SPDX-License-Identifier: GPL-3.0-or-later
//
// C ABI exports for the cryptsetup LUKS2 token plugin interface.
//
// cryptsetup loads this .so and calls these functions when it encounters
// a token of type "kbs" in the LUKS2 header.

use libc::{c_char, c_int, c_void, size_t};
use log::{error, info};
use serde_json::Value;
use std::ffi::CStr;
use std::ptr;
use std::sync::Once;
use zeroize::Zeroize;

use crate::initdata::{self};

const TOKEN_VERSION: &[u8] = b"1.0\0";
const MAX_RETRY_SECS: u64 = 120;

static INIT_LOGGER: Once = Once::new();

fn ensure_logger() {
    INIT_LOGGER.call_once(|| {
        let _ = env_logger::try_init();
    });
}

// Opaque type for libcryptsetup's crypt_device
#[repr(C)]
pub struct crypt_device {
    _opaque: [u8; 0],
}

#[link(name = "cryptsetup")]
extern "C" {
    fn crypt_log(cd: *mut crypt_device, level: c_int, msg: *const c_char);
    fn crypt_token_json_get(cd: *mut crypt_device, token: c_int, json: *mut *const c_char)
        -> c_int;
}

const CRYPT_LOG_NORMAL: c_int = 0;
const CRYPT_LOG_ERROR: c_int = 1;

fn log_to_cryptsetup(cd: *mut crypt_device, level: c_int, msg: &str) {
    let c_msg = std::ffi::CString::new(msg).unwrap_or_default();
    unsafe {
        crypt_log(cd, level, c_msg.as_ptr());
    }
}

fn do_fetch_key(cd: *mut crypt_device, token: c_int) -> Result<Vec<u8>, c_int> {
    ensure_logger();

    let mut last_err;
    let start = std::time::Instant::now();
    let mut attempt = 0u32;
    let mut json_ptr: *const c_char = ptr::null();

    let r = unsafe { crypt_token_json_get(cd, token, &mut json_ptr) };

    // TODO: to log in cryptsetup log
    let initdata = if r >= 0 && !json_ptr.is_null() {
        let json_cstr = unsafe { CStr::from_ptr(json_ptr) };
        let json_str = json_cstr.to_str().unwrap_or("");
        match initdata::new_initdata_from_json(json_str) {
            Ok(d) => d,
            Err(_) => match crate::fetch_initdata_from_smbios() {
                Ok(d) => d,
                Err(_) => {
                    error!("kbs: failed to fetch initdata from smbios");
                    return Err(-libc::EAGAIN);
                }
            },
        }
    } else {
        match crate::fetch_initdata_from_smbios() {
            Ok(d) => d,
            Err(_) => {
                error!("kbs: failed to fetch initdata from smbios");
                return Err(-libc::EAGAIN);
            }
        }
    };

    loop {
        attempt += 1;
        match crate::fetch_luks_key(&initdata) {
            Ok(key) => {
                if attempt > 1 {
                    let msg = format!(
                        "kbs: key obtained after {} seconds ({} attempts)\n",
                        start.elapsed().as_secs(),
                        attempt
                    );
                    log_to_cryptsetup(cd, CRYPT_LOG_NORMAL, &msg);
                }
                return Ok(key);
            }
            Err(e) => {
                last_err = format!("{}", e);
                let elapsed = start.elapsed().as_secs();
                if elapsed >= MAX_RETRY_SECS {
                    let msg = format!(
                        "kbs: failed after {}s ({} attempts): {}\n",
                        elapsed, attempt, last_err
                    );
                    log_to_cryptsetup(cd, CRYPT_LOG_ERROR, &msg);
                    error!("{}", msg.trim());
                    return Err(-libc::EAGAIN);
                }
                if attempt <= 3 || attempt.is_multiple_of(5) {
                    let msg = format!(
                        "kbs: attempt {} failed ({}s elapsed), retrying: {}\n",
                        attempt, elapsed, last_err
                    );
                    log_to_cryptsetup(cd, CRYPT_LOG_NORMAL, &msg);
                    info!("{}", msg.trim());
                }
                let backoff = std::cmp::min(1u64 << attempt.min(5), 32);
                std::thread::sleep(std::time::Duration::from_secs(backoff));
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn cryptsetup_token_version() -> *const c_char {
    TOKEN_VERSION.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn cryptsetup_token_open(
    cd: *mut crypt_device,
    token: c_int,
    password: *mut *mut c_char,
    password_len: *mut size_t,
    _usrptr: *mut c_void,
) -> c_int {
    log_to_cryptsetup(cd, CRYPT_LOG_NORMAL, "kbs: token plugin invoked\n");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| do_fetch_key(cd, token)));

    match result {
        Ok(Ok(mut key)) => {
            log_to_cryptsetup(
                cd,
                CRYPT_LOG_NORMAL,
                &format!("kbs: key obtained ({} bytes)\n", key.len()),
            );
            let buf = unsafe { libc::malloc(key.len()) as *mut c_char };
            if buf.is_null() {
                return -libc::ENOMEM;
            }
            unsafe {
                ptr::copy_nonoverlapping(key.as_ptr() as *const c_char, buf, key.len());
                *password = buf;
                *password_len = key.len();
            }
            key.zeroize();
            0
        }
        Ok(Err(code)) => {
            log_to_cryptsetup(
                cd,
                CRYPT_LOG_ERROR,
                &format!("kbs: fetch failed with code {}\n", code),
            );
            code
        }
        Err(_panic) => {
            log_to_cryptsetup(cd, CRYPT_LOG_ERROR, "kbs: PANIC in token plugin\n");
            -libc::EAGAIN
        }
    }
}

#[no_mangle]
pub extern "C" fn cryptsetup_token_open_pin(
    cd: *mut crypt_device,
    token: c_int,
    _pin: *const c_char,
    _pin_size: size_t,
    password: *mut *mut c_char,
    password_len: *mut size_t,
    usrptr: *mut c_void,
) -> c_int {
    cryptsetup_token_open(cd, token, password, password_len, usrptr)
}

#[no_mangle]
pub extern "C" fn cryptsetup_token_buffer_free(buffer: *mut c_void, buffer_len: size_t) {
    if !buffer.is_null() {
        unsafe {
            ptr::write_bytes(buffer as *mut u8, 0, buffer_len);
            libc::free(buffer);
        }
    }
}

#[no_mangle]
pub extern "C" fn cryptsetup_token_dump(cd: *mut crypt_device, _json: *const c_char) {
    log_to_cryptsetup(
        cd,
        CRYPT_LOG_NORMAL,
        "\ttype:       kbs\n\tmethod:     TEE attestation via KBS (trustee-attester)\n",
    );
}

/// check 'type', 'kbs' and 'resource' fields
#[no_mangle]
pub extern "C" fn cryptsetup_token_validate(_cd: *mut crypt_device, json: *const c_char) -> c_int {
    if json.is_null() {
        return -libc::EINVAL;
    }

    let json_str = match unsafe { CStr::from_ptr(json) }.to_str() {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    let v: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return -libc::EINVAL,
    };

    if v.get("type").and_then(|t| t.as_str()) != Some("kbs") {
        return -libc::EINVAL;
    }

    let has_url = v.get("trustee.kbs.url").and_then(|u| u.as_str()).is_some();
    let has_res = v
        .get("trustee.kbs.resource")
        .and_then(|r| r.as_str())
        .is_some();

    // Both present or both absent is valid; one without the other is not
    if has_url != has_res {
        return -libc::EINVAL;
    }

    // If present, neither can be empty
    if has_url {
        if v.get("trustee.kbs.url").and_then(|u| u.as_str()) == Some("") {
            return -libc::EINVAL;
        }
        if v.get("trustee.kbs.resource").and_then(|r| r.as_str()) == Some("") {
            return -libc::EINVAL;
        }
    }

    0
}
