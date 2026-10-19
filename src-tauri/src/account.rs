//! The e621 account: the API key in the credential store, and who the app is signed in as.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};

use crate::{
    api::{Api, ApiError, Credentials, models::User},
    network::Connection,
    secrets,
};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum AccountState {
    Anonymous,
    /// `id` is `None` until e621 has confirmed the saved key in this run.
    SignedIn {
        name: String,
        id: Option<u64>,
    },
    /// e621 stopped accepting the saved key (expired or revoked); it has been removed.
    Rejected {
        name: String,
    },
    /// The saved account could not be read; the app runs anonymously.
    Failed {
        code: AccountFailure,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountFailure {
    CredentialStore,
    SavedAccount,
}

/// Why signing in or out failed. The UI words it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(untagged)]
pub enum AccountError {
    #[error(transparent)]
    Api(ApiError),
    #[error("the credential store failed")]
    Store(StoreFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum StoreFailure {
    CredentialStore,
}

impl From<ApiError> for AccountError {
    fn from(error: ApiError) -> Self {
        Self::Api(error)
    }
}

/// What the credential store keeps, as one entry so the name and key cannot get out of step.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedAccount {
    username: String,
    api_key: String,
}

pub struct Account {
    api: Arc<Api>,
    state: watch::Sender<AccountState>,
    /// The confirmed user, for the blacklist (report §12, step 6).
    user: std::sync::Mutex<Option<User>>,
    /// Signing in, signing out and handling a rejection take turns.
    changing: Mutex<()>,
}

impl Account {
    pub fn new(api: Arc<Api>) -> Self {
        Self {
            api,
            state: watch::Sender::new(AccountState::Anonymous),
            user: std::sync::Mutex::new(None),
            changing: Mutex::new(()),
        }
    }

    pub fn state(&self) -> AccountState {
        self.state.borrow().clone()
    }

    pub fn watch(&self) -> watch::Receiver<AccountState> {
        self.state.subscribe()
    }

    pub fn user(&self) -> Option<User> {
        self.user.lock().expect("user lock poisoned").clone()
    }

    /// Signs in with the saved key, if there is one, and confirms it with e621 once the
    /// connection has a route (before that the proxy refuses every request). Then follows the
    /// API's rejections for as long as the app runs.
    pub async fn run(&self, connection: watch::Receiver<Connection>) {
        self.restore().await;
        // Subscribed before the confirmation goes out, so its rejection is not missed.
        let rejections = self.api.watch_rejections();
        tokio::join!(
            self.confirm_once_routed(connection),
            self.follow_rejections(rejections)
        );
    }

    async fn follow_rejections(&self, mut rejections: watch::Receiver<Option<Arc<Credentials>>>) {
        while rejections.changed().await.is_ok() {
            let rejected = rejections.borrow_and_update().clone();
            if let Some(rejected) = rejected {
                self.handle_rejection(&rejected).await;
            }
        }
    }

    async fn restore(&self) {
        let _changing = self.changing.lock().await;
        // On macOS this can wait for the user to answer a keychain access prompt.
        let saved = tokio::task::spawn_blocking(|| secrets::get(secrets::E621_ACCOUNT))
            .await
            .expect("reading the credential store panicked");
        let saved = match saved {
            Ok(Some(saved)) => saved,
            Ok(None) => return,
            Err(error) => {
                tracing::error!("could not read the saved e621 account: {error}");
                self.state.send_replace(AccountState::Failed {
                    code: AccountFailure::CredentialStore,
                });
                return;
            }
        };
        let saved: SavedAccount = match serde_json::from_str(&saved) {
            Ok(saved) => saved,
            Err(error) => {
                tracing::error!("the saved e621 account is unreadable: {error}");
                self.state.send_replace(AccountState::Failed {
                    code: AccountFailure::SavedAccount,
                });
                return;
            }
        };
        let name = saved.username.clone();
        self.api.set_credentials(Some(Arc::new(Credentials {
            username: saved.username,
            api_key: saved.api_key,
        })));
        self.state
            .send_replace(AccountState::SignedIn { name, id: None });
    }

    /// A rejection is handled by [`Account::follow_rejections`]; other failures leave the saved
    /// key in use.
    async fn confirm_once_routed(&self, mut connection: watch::Receiver<Connection>) {
        let Some(saved) = self.api.credentials() else {
            return;
        };
        // The value is not needed, and the guard it comes in must not be held across awaits.
        let routed = connection
            .wait_for(|connection| {
                matches!(connection, Connection::Direct | Connection::Tunnel { .. })
            })
            .await
            .is_ok();
        if !routed {
            return;
        }
        match self.api.me().await {
            Ok(Some(user)) => {
                let _changing = self.changing.lock().await;
                // Not if the user signed in again or out while the request was out.
                if self
                    .api
                    .credentials()
                    .is_some_and(|current| Arc::ptr_eq(&current, &saved))
                {
                    self.accept(user);
                }
            }
            Ok(None) => tracing::warn!("e621 answered the saved key as anonymous"),
            Err(error) => tracing::warn!("could not confirm the saved e621 account: {error}"),
        }
    }

    /// Checks the key with e621, then saves it, then uses it. Nothing changes if any step fails.
    pub async fn sign_in(&self, username: &str, api_key: &str) -> Result<(), AccountError> {
        let _changing = self.changing.lock().await;
        let credentials = Credentials {
            username: username.trim().to_owned(),
            api_key: api_key.trim().to_owned(),
        };
        let Some(user) = self.api.me_as(&credentials).await? else {
            return Err(ApiError::Unauthorized.into());
        };
        // Saved under the name as e621 spells it.
        let saved = SavedAccount {
            username: user.name.clone(),
            api_key: credentials.api_key,
        };
        let json = serde_json::to_string(&saved).expect("the account serializes");
        tokio::task::spawn_blocking(move || secrets::set(secrets::E621_ACCOUNT, &json))
            .await
            .expect("writing the credential store panicked")
            .map_err(|error| {
                tracing::error!("could not save the e621 account: {error}");
                AccountError::Store(StoreFailure::CredentialStore)
            })?;
        self.api.set_credentials(Some(Arc::new(Credentials {
            username: saved.username,
            api_key: saved.api_key,
        })));
        self.accept(user);
        Ok(())
    }

    /// Removes the saved key first, so a failure leaves the app signed in as before.
    pub async fn sign_out(&self) -> Result<(), AccountError> {
        let _changing = self.changing.lock().await;
        Self::forget_saved().await.map_err(|error| {
            tracing::error!("could not remove the saved e621 account: {error}");
            AccountError::Store(StoreFailure::CredentialStore)
        })?;
        self.api.set_credentials(None);
        *self.user.lock().expect("user lock poisoned") = None;
        self.state.send_replace(AccountState::Anonymous);
        Ok(())
    }

    async fn handle_rejection(&self, rejected: &Arc<Credentials>) {
        let _changing = self.changing.lock().await;
        if !self
            .api
            .credentials()
            .is_some_and(|current| Arc::ptr_eq(&current, rejected))
        {
            return;
        }
        tracing::warn!("e621 rejected the saved key of {}", rejected.username);
        self.api.set_credentials(None);
        *self.user.lock().expect("user lock poisoned") = None;
        // The key no longer works either way; if it stays saved, the next start rejects it again.
        if let Err(error) = Self::forget_saved().await {
            tracing::error!("could not remove the rejected e621 account: {error}");
        }
        self.state.send_replace(AccountState::Rejected {
            name: rejected.username.clone(),
        });
    }

    fn accept(&self, user: User) {
        tracing::info!("signed in to e621 as {} ({})", user.name, user.id);
        self.state.send_replace(AccountState::SignedIn {
            name: user.name.clone(),
            id: Some(user.id),
        });
        *self.user.lock().expect("user lock poisoned") = Some(user);
    }

    async fn forget_saved() -> Result<(), keyring_core::Error> {
        tokio::task::spawn_blocking(|| secrets::delete(secrets::E621_ACCOUNT))
            .await
            .expect("writing the credential store panicked")
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use keyring_core::mock;
    use serde_json::{Value, json};

    use super::*;
    use crate::{
        api::testing::{Received, json_response, ok, serve},
        network::{USER_AGENT, UserAgent},
        secrets::testing::{entry, mock_store},
    };

    fn user(id: u64, name: &str) -> Value {
        let mut user: Value =
            serde_json::from_str(include_str!("../tests/fixtures/users_me_anonymous.json"))
                .unwrap();
        user["id"] = json!(id);
        user["name"] = json!(name);
        user
    }

    fn unauthorized() -> String {
        json_response("401 Unauthorized", r#"{"success":false}"#)
    }

    struct Fixture {
        account: Arc<Account>,
        user_agent: UserAgent,
        received: tokio::sync::mpsc::UnboundedReceiver<Received>,
    }

    async fn fixture(responses: Vec<String>) -> Fixture {
        let (base, received) = serve(responses).await;
        let user_agent = UserAgent::new();
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let api = Arc::new(Api::with_base(http, user_agent.clone(), base));
        Fixture {
            account: Arc::new(Account::new(api)),
            user_agent,
            received,
        }
    }

    fn save(username: &str, api_key: &str) {
        entry(secrets::E621_ACCOUNT)
            .set_password(&json!({ "username": username, "apiKey": api_key }).to_string())
            .unwrap();
    }

    fn saved() -> Option<Value> {
        secrets::get(secrets::E621_ACCOUNT)
            .unwrap()
            .map(|saved| serde_json::from_str(&saved).unwrap())
    }

    /// Runs the account with the given connection state and waits until its state matches.
    async fn run_until(
        account: &Arc<Account>,
        connection: Connection,
        done: impl Fn(&AccountState) -> bool,
    ) -> AccountState {
        let (_connection, watch) = watch::channel(connection);
        let running = Arc::clone(account);
        let mut states = account.watch();
        let task = tokio::spawn(async move { running.run(watch).await });
        let state =
            tokio::time::timeout(Duration::from_secs(5), states.wait_for(|state| done(state)))
                .await
                .expect("the account never reached the expected state")
                .unwrap()
                .clone();
        task.abort();
        state
    }

    #[tokio::test]
    async fn signs_in_with_the_name_as_e621_spells_it() {
        let _store = mock_store().await;
        let mut fixture = fixture(vec![ok(&user(42, "Alice"))]).await;
        fixture.account.sign_in(" alice ", " key ").await.unwrap();

        assert_eq!(
            fixture.account.state(),
            AccountState::SignedIn {
                name: "Alice".into(),
                id: Some(42)
            }
        );
        assert_eq!(
            saved(),
            Some(json!({ "username": "Alice", "apiKey": "key" }))
        );
        assert_eq!(
            fixture.user_agent.get(),
            crate::network::user_agent_for("Alice")
        );
        assert_eq!(
            fixture
                .received
                .recv()
                .await
                .unwrap()
                .authorization
                .as_deref(),
            // base64("alice:key")
            Some("Basic YWxpY2U6a2V5")
        );
    }

    #[tokio::test]
    async fn changes_nothing_when_e621_rejects_the_key() {
        let _store = mock_store().await;
        let fixture = fixture(vec![unauthorized()]).await;
        assert_eq!(
            fixture.account.sign_in("alice", "wrong").await,
            Err(AccountError::Api(ApiError::Unauthorized))
        );
        assert_eq!(fixture.account.state(), AccountState::Anonymous);
        assert_eq!(saved(), None);
        assert_eq!(fixture.user_agent.get(), USER_AGENT);
    }

    #[tokio::test]
    async fn stays_signed_out_when_the_key_cannot_be_saved() {
        let _store = mock_store().await;
        let account_entry = entry(secrets::E621_ACCOUNT);
        let credential: &mock::Cred = account_entry.as_any().downcast_ref().unwrap();
        credential.set_error(keyring_core::Error::NoStorageAccess("denied".into()));
        let fixture = fixture(vec![ok(&user(42, "Alice"))]).await;
        assert_eq!(
            fixture.account.sign_in("alice", "key").await,
            Err(AccountError::Store(StoreFailure::CredentialStore))
        );
        assert_eq!(fixture.account.state(), AccountState::Anonymous);
        assert_eq!(fixture.user_agent.get(), USER_AGENT);
    }

    #[tokio::test]
    async fn signs_out_and_forgets_the_key() {
        let _store = mock_store().await;
        let fixture = fixture(vec![ok(&user(42, "Alice"))]).await;
        fixture.account.sign_in("alice", "key").await.unwrap();
        fixture.account.sign_out().await.unwrap();
        assert_eq!(fixture.account.state(), AccountState::Anonymous);
        assert_eq!(saved(), None);
        assert_eq!(fixture.account.user(), None);
        assert_eq!(fixture.user_agent.get(), USER_AGENT);
    }

    #[tokio::test]
    async fn restores_the_saved_key_and_confirms_it_once_routed() {
        let _store = mock_store().await;
        save("Alice", "key");
        let mut fixture = fixture(vec![ok(&user(42, "Alice"))]).await;
        let state = run_until(&fixture.account, Connection::Direct, |state| {
            matches!(state, AccountState::SignedIn { id: Some(_), .. })
        })
        .await;
        assert_eq!(
            state,
            AccountState::SignedIn {
                name: "Alice".into(),
                id: Some(42)
            }
        );
        let request = fixture.received.recv().await.unwrap();
        assert_eq!(request.target, "/users/me.json");
        assert_eq!(request.user_agent, crate::network::user_agent_for("Alice"));
    }

    #[tokio::test]
    async fn waits_for_a_route_before_confirming() {
        let _store = mock_store().await;
        save("Alice", "key");
        let mut fixture = fixture(Vec::new()).await;
        let state = run_until(&fixture.account, Connection::Connecting, |state| {
            matches!(state, AccountState::SignedIn { .. })
        })
        .await;
        assert_eq!(
            state,
            AccountState::SignedIn {
                name: "Alice".into(),
                id: None
            }
        );
        assert!(fixture.received.try_recv().is_err());
    }

    #[tokio::test]
    async fn forgets_a_saved_key_that_e621_no_longer_accepts() {
        let _store = mock_store().await;
        save("Alice", "expired");
        let fixture = fixture(vec![unauthorized()]).await;
        let state = run_until(&fixture.account, Connection::Direct, |state| {
            matches!(state, AccountState::Rejected { .. })
        })
        .await;
        assert_eq!(
            state,
            AccountState::Rejected {
                name: "Alice".into()
            }
        );
        assert_eq!(saved(), None);
        assert_eq!(fixture.user_agent.get(), USER_AGENT);
    }

    #[tokio::test]
    async fn reports_an_unreadable_saved_account() {
        let _store = mock_store().await;
        entry(secrets::E621_ACCOUNT)
            .set_password("not json")
            .unwrap();
        let fixture = fixture(Vec::new()).await;
        let state = run_until(&fixture.account, Connection::Direct, |state| {
            matches!(state, AccountState::Failed { .. })
        })
        .await;
        assert_eq!(
            state,
            AccountState::Failed {
                code: AccountFailure::SavedAccount
            }
        );
    }

    #[tokio::test]
    async fn reports_a_failing_credential_store() {
        let _store = mock_store().await;
        let account_entry = entry(secrets::E621_ACCOUNT);
        let credential: &mock::Cred = account_entry.as_any().downcast_ref().unwrap();
        credential.set_error(keyring_core::Error::NoStorageAccess("denied".into()));
        let fixture = fixture(Vec::new()).await;
        let state = run_until(&fixture.account, Connection::Direct, |state| {
            matches!(state, AccountState::Failed { .. })
        })
        .await;
        assert_eq!(
            state,
            AccountState::Failed {
                code: AccountFailure::CredentialStore
            }
        );
    }
}
