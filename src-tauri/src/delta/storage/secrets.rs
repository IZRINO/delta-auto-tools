use base64::{engine::general_purpose::STANDARD, Engine};

use crate::delta::error::DeltaError;

const SECRET_PREFIX: &str = "dpapi:v1:";

pub fn seal_secret(value: &str) -> Result<String, DeltaError> {
    if value.starts_with(SECRET_PREFIX) {
        return Ok(value.to_string());
    }

    let encrypted = platform::protect(value.as_bytes()).map_err(DeltaError::Storage)?;
    Ok(format!("{SECRET_PREFIX}{}", STANDARD.encode(encrypted)))
}

pub fn open_secret(value: &str) -> Result<String, DeltaError> {
    let Some(encoded) = value.strip_prefix(SECRET_PREFIX) else {
        return Ok(value.to_string());
    };

    let encrypted = STANDARD
        .decode(encoded)
        .map_err(|error| DeltaError::Storage(format!("无法解码本地凭据: {error}")))?;
    let decrypted = platform::unprotect(&encrypted).map_err(DeltaError::Storage)?;
    String::from_utf8(decrypted)
        .map_err(|error| DeltaError::Storage(format!("本地凭据不是有效 UTF-8: {error}")))
}

pub fn is_sealed(value: &str) -> bool {
    value.starts_with(SECRET_PREFIX)
}

#[cfg(test)]
pub fn test_seal_secret(value: &str) -> Result<String, DeltaError> {
    seal_secret(value)
}

#[cfg(target_os = "windows")]
mod platform {
    use std::{ptr, slice};

    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };

    pub fn protect(value: &[u8]) -> Result<Vec<u8>, String> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: value.len() as u32,
            pbData: value.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };

        // SAFETY: DATA_BLOB points at the immutable input slice for this call only;
        // all optional pointer parameters are null as permitted by Win32 API, and output is initialized by CryptProtectData.
        let ok = unsafe {
            CryptProtectData(
                &input,
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(format!(
                "加密本地凭据失败: {}",
                std::io::Error::last_os_error()
            ));
        }

        blob_to_vec_and_free(output)
    }

    pub fn unprotect(value: &[u8]) -> Result<Vec<u8>, String> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: value.len() as u32,
            pbData: value.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };

        // SAFETY: DATA_BLOB points at the encrypted input slice for this call only;
        // all optional pointer parameters are null as permitted by Win32 API, and output is initialized by CryptUnprotectData.
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(format!(
                "解密本地凭据失败: {}",
                std::io::Error::last_os_error()
            ));
        }

        blob_to_vec_and_free(output)
    }

    fn blob_to_vec_and_free(output: CRYPT_INTEGER_BLOB) -> Result<Vec<u8>, String> {
        if output.pbData.is_null() {
            return Err("本地凭据接口返回空数据".to_string());
        }

        // SAFETY: output.pbData is checked non-null and output.cbData is the byte length returned by Win32.
        let bytes =
            unsafe { slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        // SAFETY: output.pbData was allocated by CryptProtectData/CryptUnprotectData and must be freed with LocalFree once.
        unsafe {
            let _ = LocalFree(output.pbData as _);
        }
        Ok(bytes)
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    pub fn protect(_: &[u8]) -> Result<Vec<u8>, String> {
        Err("当前平台不支持本地凭据加密".to_string())
    }

    pub fn unprotect(_: &[u8]) -> Result<Vec<u8>, String> {
        Err("当前平台不支持本地凭据解密".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{is_sealed, open_secret, seal_secret};

    #[test]
    fn seals_and_opens_secret() {
        let sealed = seal_secret("token-abc").unwrap();

        assert!(is_sealed(&sealed));
        assert_ne!(sealed, "token-abc");
        assert_eq!(open_secret(&sealed).unwrap(), "token-abc");
    }

    #[test]
    fn opens_legacy_plaintext_secret() {
        assert_eq!(open_secret("plain").unwrap(), "plain");
    }
}
