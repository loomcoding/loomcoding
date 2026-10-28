#[cfg(windows)]
pub fn get(name: &str) -> anyhow::Result<Option<String>> {
    use std::slice;
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };

    let target = wide_target(name);
    let mut credential = std::ptr::null_mut::<CREDENTIALW>();
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
        let error = unsafe { GetLastError() };
        if error == ERROR_NOT_FOUND {
            return Ok(None);
        }
        return Err(std::io::Error::from_raw_os_error(error as i32).into());
    }

    let entry = unsafe { &*credential };
    let value: anyhow::Result<String> = if entry.CredentialBlobSize == 0 {
        Ok(String::new())
    } else if entry.CredentialBlob.is_null() {
        anyhow::bail!("Windows Credential Manager returned an empty credential blob")
    } else {
        let bytes = unsafe {
            slice::from_raw_parts(entry.CredentialBlob, entry.CredentialBlobSize as usize)
        };
        String::from_utf8(bytes.to_vec()).map_err(Into::into)
    };
    unsafe { CredFree(credential.cast()) };
    Ok(Some(value?))
}

#[cfg(windows)]
pub fn set(name: &str, value: &str) -> anyhow::Result<()> {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Security::Credentials::{
        CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    let target = wide_target(name);
    let username: Vec<u16> = "CodeLoom".encode_utf16().chain(Some(0)).collect();
    let value = value.as_bytes();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_ptr() as *mut u16,
        CredentialBlobSize: value.len() as u32,
        CredentialBlob: value.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: username.as_ptr() as *mut u16,
        ..Default::default()
    };
    if unsafe { CredWriteW(&credential, 0) } == 0 {
        return Err(std::io::Error::from_raw_os_error(unsafe { GetLastError() } as i32).into());
    }
    Ok(())
}

#[cfg(windows)]
fn wide_target(name: &str) -> Vec<u16> {
    format!("CodeLoom:{name}")
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

#[cfg(not(windows))]
pub fn get(_name: &str) -> anyhow::Result<Option<String>> {
    Ok(None)
}

#[cfg(not(windows))]
pub fn set(_name: &str, _value: &str) -> anyhow::Result<()> {
    anyhow::bail!("secure API key saving is only available on Windows; use the configured environment variable")
}

pub fn get_or_create(name: &str) -> anyhow::Result<String> {
    if let Some(value) = get(name)?.filter(|value| !value.trim().is_empty()) {
        return Ok(value);
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let secret = hex(&bytes);
    set(name, &secret)
        .map_err(|error| anyhow::anyhow!("could not store A2A credential securely: {error}"))?;
    Ok(secret)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_credential_is_256_bits_of_hex_text() {
        let token = hex(&[0xab; 32]);
        assert_eq!(token, "ab".repeat(32));
    }
}
