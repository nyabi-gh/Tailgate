//! New versions from the GitHub releases (docs/report.md §10). The requests take the app's
//! proxy like every other, so the kill switch holds for them too.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::network::{CONNECT_TIMEOUT, READ_TIMEOUT};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub version: String,
    pub notes: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum UpdateError {
    #[error("could not reach the update server")]
    Network,
    /// The release is missing, malformed, or its signature does not match the app's key.
    #[error("the release could not be read or verified")]
    InvalidRelease,
    #[error("could not install the update")]
    Install,
}

impl From<tauri_plugin_updater::Error> for UpdateError {
    fn from(error: tauri_plugin_updater::Error) -> Self {
        use tauri_plugin_updater::Error;
        tracing::warn!("update failed: {error}");
        match error {
            Error::Reqwest(_) | Error::Network(_) => Self::Network,
            Error::ReleaseNotFound
            | Error::TargetNotFound(_)
            | Error::TargetsNotFound(_)
            | Error::Serialization(_)
            | Error::Semver(_)
            | Error::UrlParse(_)
            | Error::Minisign(_)
            | Error::Base64(_)
            | Error::SignatureUtf8(_)
            | Error::SignedVersionMismatch { .. }
            | Error::MissingSignedVersion
            | Error::InvalidUpdaterFormat => Self::InvalidRelease,
            _ => Self::Install,
        }
    }
}

/// Download progress in bytes; `total` is unknown when the server sends no length.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub received: u64,
    pub total: Option<u64>,
}

/// The update found by the last check, kept for the install that may follow.
#[derive(Default)]
pub struct Updates {
    found: Mutex<Option<Update>>,
}

impl Updates {
    pub async fn check(
        &self,
        app: &AppHandle,
        proxy: &str,
    ) -> Result<Option<Available>, UpdateError> {
        let proxy = Url::parse(proxy).map_err(|error| {
            tracing::error!("the app's proxy address is not a URL: {error}");
            UpdateError::Network
        })?;
        let update = app
            .updater_builder()
            .proxy(proxy)
            // Limits between reads rather than for the whole request, which would also cut off
            // a slow download of the new version.
            .configure_client(|client| {
                client
                    .connect_timeout(CONNECT_TIMEOUT)
                    .read_timeout(READ_TIMEOUT)
            })
            .build()?
            .check()
            .await?;
        let available = update.as_ref().map(|update| Available {
            version: update.version.clone(),
            notes: update.body.clone().filter(|notes| !notes.trim().is_empty()),
        });
        *self.lock() = update;
        Ok(available)
    }

    /// On Windows the installer takes over and the app exits; on macOS the new version runs
    /// after a restart.
    pub async fn install(&self, mut progress: impl FnMut(Progress)) -> Result<(), UpdateError> {
        let Some(update) = self.lock().clone() else {
            tracing::error!("install was asked for without an update found");
            return Err(UpdateError::Install);
        };
        let mut received = 0u64;
        update
            .download_and_install(
                |chunk, total| {
                    received += chunk as u64;
                    progress(Progress { received, total });
                },
                || {},
            )
            .await?;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Update>> {
        self.found.lock().expect("update lock poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_are_told_apart_for_the_user() {
        use tauri_plugin_updater::Error;
        assert_eq!(
            UpdateError::from(Error::Network("timed out".into())),
            UpdateError::Network
        );
        assert_eq!(
            UpdateError::from(Error::ReleaseNotFound),
            UpdateError::InvalidRelease
        );
        assert_eq!(
            UpdateError::from(Error::SignatureUtf8("bad".into())),
            UpdateError::InvalidRelease
        );
        assert_eq!(
            UpdateError::from(Error::TempDirNotFound),
            UpdateError::Install
        );
    }
}
