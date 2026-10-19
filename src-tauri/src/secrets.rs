//! Secrets live only in the OS credential store (macOS Keychain, Windows Credential Manager).

use keyring_core::{Entry, Error};

pub(crate) const SERVICE: &str = "pro.tb.nyabi.Tailgate";

pub const MULLVAD_ACCOUNT: &str = "mullvad-account";
pub const MULLVAD_DEVICE: &str = "mullvad-device";
pub const E621_ACCOUNT: &str = "e621-account";

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

#[cfg(test)]
pub(crate) mod testing {
    use keyring_core::{Entry, mock};
    use tokio::sync::{Mutex, MutexGuard};

    use super::SERVICE;

    /// The default credential store is process-wide, so the tests that set one take turns.
    static CREDENTIAL_STORE: Mutex<()> = Mutex::const_new(());

    /// Installs an empty mock store, the test's own while the guard lives.
    pub(crate) async fn mock_store() -> MutexGuard<'static, ()> {
        let guard = CREDENTIAL_STORE.lock().await;
        keyring_core::set_default_store(mock::Store::new().unwrap());
        guard
    }

    pub(crate) fn entry(name: &str) -> Entry {
        Entry::new(SERVICE, name).unwrap()
    }
}
