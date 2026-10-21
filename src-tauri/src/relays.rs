//! Mullvad's relay list, kept on disk: downloading it was most of the 3–4 s a tunnel took to start
//! (docs/research.md). A saved copy is used at once and refreshed after the tunnel is up.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::tunnel::mullvad::{Fetched, Mullvad, MullvadError, Relay, RelayList};

#[derive(Deserialize, Serialize)]
struct Saved {
    etag: Option<String>,
    list: RelayList,
}

/// A city the user can pick: one that has at least one active relay.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationChoice {
    pub code: String,
    pub country: String,
    pub city: String,
}

/// Where a list came from, which decides whether a failure may be the list's fault.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origin {
    Saved,
    Fetched,
}

pub struct Relays {
    path: PathBuf,
    current: Mutex<Option<(Arc<RelayList>, Option<String>)>>,
}

impl Relays {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            current: Mutex::new(None),
        }
    }

    /// The list in memory, else the saved copy, else a download.
    pub async fn get(&self, mullvad: &Mullvad) -> Result<(Arc<RelayList>, Origin), MullvadError> {
        if let Some((list, _)) = self.lock().clone() {
            return Ok((list, Origin::Saved));
        }
        if let Some(saved) = self.read_saved().await {
            let list = Arc::new(saved.list);
            *self.lock() = Some((Arc::clone(&list), saved.etag));
            return Ok((list, Origin::Saved));
        }
        Ok((self.refresh(mullvad).await?, Origin::Fetched))
    }

    /// Asks Mullvad for a newer list than the one at hand and saves what comes back.
    pub async fn refresh(&self, mullvad: &Mullvad) -> Result<Arc<RelayList>, MullvadError> {
        let known = self.lock().clone();
        let etag = known.as_ref().and_then(|(_, etag)| etag.clone());
        match mullvad.relays_since(etag.as_deref()).await? {
            Fetched::NotModified => {
                let (list, _) = known.expect("a tag is only sent along with a list");
                Ok(list)
            }
            Fetched::Changed { list, etag } => {
                let list = Arc::new(list);
                *self.lock() = Some((Arc::clone(&list), etag.clone()));
                self.save(Saved {
                    etag,
                    list: RelayList::clone(&list),
                })
                .await;
                Ok(list)
            }
        }
    }

    /// A damaged or unreadable copy only costs a download, so it is logged and skipped.
    async fn read_saved(&self) -> Option<Saved> {
        let path = self.path.clone();
        let text = tokio::task::spawn_blocking(move || std::fs::read_to_string(path))
            .await
            .expect("reading the relay list panicked");
        let text = match text {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => {
                tracing::warn!("could not read the saved relay list: {error}");
                return None;
            }
        };
        serde_json::from_str(&text)
            .inspect_err(|error| tracing::warn!("the saved relay list is damaged: {error}"))
            .ok()
    }

    async fn save(&self, saved: Saved) {
        let path = self.path.clone();
        let written = tokio::task::spawn_blocking(move || {
            if let Some(directory) = path.parent() {
                std::fs::create_dir_all(directory)?;
            }
            let temporary = path.with_extension("json.tmp");
            std::fs::write(
                &temporary,
                serde_json::to_vec(&saved).expect("the relay list serializes"),
            )?;
            std::fs::rename(&temporary, &path)
        })
        .await
        .expect("saving the relay list panicked");
        if let Err(error) = written {
            tracing::warn!("could not save the relay list: {error}");
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<(Arc<RelayList>, Option<String>)>> {
        self.current.lock().expect("relay list lock poisoned")
    }
}

/// Picks one of the location's active relays at random, as Mullvad's own app does, so users of
/// a city spread over its servers.
pub fn pick<'a>(list: &'a RelayList, location: &str) -> Option<&'a Relay> {
    let candidates: Vec<&Relay> = list
        .relays
        .iter()
        .filter(|relay| relay.active && relay.location == location)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let index = getrandom::u32().expect("the OS random source is unavailable") as usize;
    Some(candidates[index % candidates.len()])
}

pub fn choices(list: &RelayList) -> Vec<LocationChoice> {
    let mut choices: Vec<LocationChoice> = list
        .locations
        .iter()
        .filter(|(code, _)| {
            list.relays
                .iter()
                .any(|relay| relay.active && &relay.location == *code)
        })
        .map(|(code, location)| LocationChoice {
            code: code.clone(),
            country: location.country.clone(),
            city: location.city.clone(),
        })
        .collect();
    choices.sort_by(|a, b| (&a.country, &a.city).cmp(&(&b.country, &b.city)));
    choices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::testing::{Received, json_response, serve},
        settings::testing::TempDir,
    };

    const FIXTURE: &str = include_str!("../tests/fixtures/mullvad_relays.json");

    fn fixture() -> RelayList {
        #[derive(Deserialize)]
        struct Response {
            locations: std::collections::BTreeMap<String, crate::tunnel::mullvad::Location>,
            wireguard: WireGuard,
        }
        #[derive(Deserialize)]
        struct WireGuard {
            relays: Vec<Relay>,
        }
        let response: Response = serde_json::from_str(FIXTURE).unwrap();
        RelayList {
            relays: response.wireguard.relays,
            locations: response.locations,
        }
    }

    fn with_etag(etag: &str) -> String {
        json_response("200 OK", FIXTURE).replacen("\r\n", &format!("\r\netag: {etag}\r\n"), 1)
    }

    async fn fake_mullvad(
        responses: Vec<String>,
    ) -> (Mullvad, tokio::sync::mpsc::UnboundedReceiver<Received>) {
        let (base, received) = serve(responses).await;
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (Mullvad::with_base(http, base), received)
    }

    #[test]
    fn offers_only_cities_with_an_active_relay() {
        let choices = choices(&fixture());
        let codes: Vec<&str> = choices.iter().map(|choice| choice.code.as_str()).collect();
        // br-sao has only an inactive relay in the fixture.
        assert_eq!(codes, ["jp-osa", "jp-tyo", "se-sto", "us-nyc"]);
        assert_eq!(choices[1].country, "Japan");
        assert_eq!(choices[1].city, "Tokyo");
    }

    #[test]
    fn picks_an_active_relay_of_the_location() {
        let list = fixture();
        for _ in 0..20 {
            let relay = pick(&list, "jp-tyo").unwrap();
            assert!(relay.active && relay.location == "jp-tyo");
        }
        assert!(pick(&list, "br-sao").is_none());
        assert!(pick(&list, "jp").is_none());
    }

    #[tokio::test]
    async fn saves_the_list_and_reads_it_back_without_a_download() {
        let dir = TempDir::new();
        let (mullvad, mut received) = fake_mullvad(vec![with_etag("\"v1\"")]).await;
        let (list, origin) = Relays::new(dir.join("relays.json"))
            .get(&mullvad)
            .await
            .unwrap();
        assert_eq!(origin, Origin::Fetched);
        assert_eq!(list.relays.len(), 9);
        assert!(received.recv().await.is_some());

        // A fresh run: no server answers, so only the saved copy can serve this.
        let (offline, _) = fake_mullvad(vec![]).await;
        let (list, origin) = Relays::new(dir.join("relays.json"))
            .get(&offline)
            .await
            .unwrap();
        assert_eq!(origin, Origin::Saved);
        assert_eq!(list.relays.len(), 9);
    }

    #[tokio::test]
    async fn refreshing_sends_the_saved_tag_and_keeps_the_list_when_unchanged() {
        let dir = TempDir::new();
        let (mullvad, mut received) = fake_mullvad(vec![
            with_etag("\"v1\""),
            "HTTP/1.1 304 Not Modified\r\ncontent-length: 0\r\n\r\n".into(),
        ])
        .await;
        let relays = Relays::new(dir.join("relays.json"));
        relays.get(&mullvad).await.unwrap();
        let list = relays.refresh(&mullvad).await.unwrap();
        assert_eq!(list.relays.len(), 9);
        received.recv().await.unwrap();
        let second = received.recv().await.unwrap();
        assert_eq!(second.header("if-none-match"), Some("\"v1\""));
    }

    #[tokio::test]
    async fn a_damaged_copy_is_replaced_by_a_download() {
        let dir = TempDir::new();
        std::fs::write(dir.join("relays.json"), "{").unwrap();
        let (mullvad, _) = fake_mullvad(vec![with_etag("\"v2\"")]).await;
        let (_, origin) = Relays::new(dir.join("relays.json"))
            .get(&mullvad)
            .await
            .unwrap();
        assert_eq!(origin, Origin::Fetched);
    }
}
