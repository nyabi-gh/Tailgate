//! Secrets live only in the OS credential store (macOS Keychain, Windows Credential Manager).

use keyring_core::{Entry, Error};

const SERVICE: &str = "pro.tb.nyabi.Tailgate";

pub const MULLVAD_ACCOUNT: &str = "mullvad-account";
pub const MULLVAD_DEVICE: &str = "mullvad-device";

/// Must run once before any other call in this module.
pub fn init() -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    keyring_core::set_default_store(apple_native_keyring_store::keychain::Store::new()?);
    #[cfg(windows)]
    keyring_core::set_default_store(windows_native_keyring_store::Store::new()?);
    Ok(())
}

pub fn get(name: &str) -> Result<Option<String>, Error> {
    match Entry::new(SERVICE, name)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn set(name: &str, value: &str) -> Result<(), Error> {
    Entry::new(SERVICE, name)?.set_password(value)
}

pub fn delete(name: &str) -> Result<(), Error> {
    match Entry::new(SERVICE, name)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(error),
    }
}
