//! The tag searches the UI has open, each read page by page.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde::Serialize;
use tokio::{sync::Mutex, time::Instant};

use crate::{
    api::{
        Api, ApiError, Credentials, PostSearch,
        models::{Post, TagCategory, Tags},
    },
    blacklist::Blacklist,
};

/// A page whose posts are all blacklisted is followed by the next, up to this many pages per
/// call (one second each under the throttle); then the empty result goes back and the grid
/// asks again as it scrolls.
const MAX_PAGES_PER_CALL: usize = 5;
/// How long a fetched blacklist stands before the window coming back to the front asks e621
/// again: the user may have changed it on the site meanwhile.
const BLACKLIST_REFRESH: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    /// Blacklisted posts included: the UI hides them until asked to show them.
    pub posts: Vec<Post>,
    pub blacklisted: Vec<u64>,
    /// Which blacklist marked them, so the UI can tell a page marked before a refresh it has
    /// already applied (see [`BlacklistChange`]).
    pub blacklist_generation: u64,
    pub done: bool,
    /// Categories of the searched tags that this page's posts reveal, so the search field can
    /// color them without asking e621.
    pub categories: HashMap<String, TagCategory>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(untagged)]
pub enum SearchError {
    #[error(transparent)]
    Api(ApiError),
    #[error("the search was closed")]
    Closed(ClosedSearch),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum ClosedSearch {
    Closed,
}

/// The screen a search belongs to. Each keeps one search open, so going back to a screen finds
/// its results where they were.
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Search,
    Favorites,
}

struct OpenSearch {
    screen: Screen,
    tags: Vec<String>,
    pages: Mutex<PostSearch>,
    /// Every post handed to the UI, so a changed blacklist can mark them anew.
    loaded: std::sync::Mutex<Vec<Post>>,
}

/// The blacklist of the credentials it was fetched with, so signing in or out fetches anew.
struct CachedBlacklist {
    credentials: Option<Arc<Credentials>>,
    source: String,
    blacklist: Arc<Blacklist>,
    generation: u64,
    checked_at: Instant,
}

/// What a refreshed blacklist changed: every open search's blacklisted posts, all of them.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlacklistChange {
    pub generation: u64,
    pub searches: Vec<SearchBlacklisted>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchBlacklisted {
    pub id: u64,
    pub blacklisted: Vec<u64>,
}

#[derive(Default)]
pub struct Searches {
    next_id: AtomicU64,
    open: std::sync::Mutex<HashMap<u64, Arc<OpenSearch>>>,
    /// Held while a page is marked and recorded, so a refresh never falls between the two.
    blacklist: Mutex<Option<CachedBlacklist>>,
    blacklist_generation: AtomicU64,
}

impl Searches {
    pub fn start(&self, tags: &str) -> u64 {
        self.open(OpenSearch {
            screen: Screen::Search,
            tags: tags.split_whitespace().map(str::to_owned).collect(),
            pages: Mutex::new(PostSearch::new(tags)),
            loaded: std::sync::Mutex::default(),
        })
    }

    /// The signed-in user's favorites, newest first.
    pub fn start_favorites(&self) -> u64 {
        self.open(OpenSearch {
            screen: Screen::Favorites,
            tags: Vec::new(),
            pages: Mutex::new(PostSearch::favorites()),
            loaded: std::sync::Mutex::default(),
        })
    }

    /// Closes the search the same screen had open before.
    fn open(&self, search: OpenSearch) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut open = self.open.lock().expect("searches lock poisoned");
        open.retain(|_, other| other.screen != search.screen);
        open.insert(id, Arc::new(search));
        id
    }

    pub async fn next(&self, api: &Api, id: u64) -> Result<SearchPage, SearchError> {
        let search = self
            .open
            .lock()
            .expect("searches lock poisoned")
            .get(&id)
            .cloned()
            .ok_or(SearchError::Closed(ClosedSearch::Closed))?;
        let mut cached = self.blacklist.lock().await;
        // Before any post is returned, so none slips through unfiltered.
        let (blacklist, generation) = self
            .current_blacklist(&mut cached, api)
            .await
            .map_err(SearchError::Api)?;
        let mut pages = search.pages.lock().await;
        let mut posts = Vec::new();
        let mut blacklisted = Vec::new();
        for _ in 0..MAX_PAGES_PER_CALL {
            let page = match pages.next_page(api).await {
                Ok(page) => page,
                Err(error) if posts.is_empty() => return Err(SearchError::Api(error)),
                // The hidden posts already in hand go back; the next call asks for this page
                // again and reports the error if it persists.
                Err(error) => {
                    tracing::warn!("stopped following blacklisted pages: {error}");
                    break;
                }
            };
            let mut any_visible = false;
            for post in page {
                if blacklist.matches(&post) {
                    blacklisted.push(post.id);
                } else {
                    any_visible = true;
                }
                posts.push(post);
            }
            if any_visible || pages.is_done() {
                break;
            }
        }
        search
            .loaded
            .lock()
            .expect("loaded posts lock poisoned")
            .extend(posts.iter().cloned());
        Ok(SearchPage {
            categories: categories(&search.tags, &posts),
            done: pages.is_done(),
            blacklisted,
            blacklist_generation: generation,
            posts,
        })
    }

    /// The blacklist for the API's current credentials, fetched when there is none for them yet.
    async fn current_blacklist(
        &self,
        cached: &mut Option<CachedBlacklist>,
        api: &Api,
    ) -> Result<(Arc<Blacklist>, u64), ApiError> {
        let credentials = api.credentials();
        if let Some(cached) = cached.as_ref()
            && same_credentials(cached.credentials.as_ref(), credentials.as_ref())
        {
            return Ok((Arc::clone(&cached.blacklist), cached.generation));
        }
        let source = api.blacklist().await?;
        let blacklist = Arc::new(Blacklist::parse(&source));
        let generation = self.blacklist_generation.fetch_add(1, Ordering::Relaxed) + 1;
        *cached = Some(CachedBlacklist {
            credentials,
            source,
            blacklist: Arc::clone(&blacklist),
            generation,
            checked_at: Instant::now(),
        });
        Ok((blacklist, generation))
    }

    /// Asks e621 for the blacklist again if it was last asked [`BLACKLIST_REFRESH`] ago or more,
    /// and when it changed, marks the posts every open search has loaded anew. Before the first
    /// page, or after the credentials changed, there is nothing to refresh: the next page fetches
    /// the blacklist it needs.
    pub async fn refresh_blacklist(&self, api: &Api) -> Result<Option<BlacklistChange>, ApiError> {
        let mut cached = self.blacklist.lock().await;
        let Some(current) = cached.as_mut() else {
            return Ok(None);
        };
        if !same_credentials(current.credentials.as_ref(), api.credentials().as_ref())
            || current.checked_at.elapsed() < BLACKLIST_REFRESH
        {
            return Ok(None);
        }
        // Also when the request fails, so an unreachable e621 is not asked on every focus.
        current.checked_at = Instant::now();
        let source = api.blacklist().await?;
        if source == current.source {
            return Ok(None);
        }
        current.blacklist = Arc::new(Blacklist::parse(&source));
        current.source = source;
        current.generation = self.blacklist_generation.fetch_add(1, Ordering::Relaxed) + 1;

        let open: Vec<(u64, Arc<OpenSearch>)> = self
            .open
            .lock()
            .expect("searches lock poisoned")
            .iter()
            .map(|(id, search)| (*id, Arc::clone(search)))
            .collect();
        let searches = open
            .into_iter()
            .map(|(id, search)| SearchBlacklisted {
                id,
                blacklisted: search
                    .loaded
                    .lock()
                    .expect("loaded posts lock poisoned")
                    .iter()
                    .filter(|post| current.blacklist.matches(post))
                    .map(|post| post.id)
                    .collect(),
            })
            .collect();
        Ok(Some(BlacklistChange {
            generation: current.generation,
            searches,
        }))
    }

    pub fn close(&self, id: u64) {
        self.open
            .lock()
            .expect("searches lock poisoned")
            .remove(&id);
    }
}

fn same_credentials(a: Option<&Arc<Credentials>>, b: Option<&Arc<Credentials>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// Each searched tag (without its `-` or `~`) found in a post's tag lists, with the category of
/// the list it is in. Metatags and wildcards never match a tag and are left out.
fn categories(tags: &[String], posts: &[Post]) -> HashMap<String, TagCategory> {
    let mut found = HashMap::new();
    for tag in tags {
        let name = tag.trim_start_matches(['-', '~']).to_ascii_lowercase();
        if found.contains_key(&name) {
            continue;
        }
        if let Some(category) = posts.iter().find_map(|post| category_of(&post.tags, &name)) {
            found.insert(name, category);
        }
    }
    found
}

fn category_of(tags: &Tags, name: &str) -> Option<TagCategory> {
    [
        (&tags.general, TagCategory::General),
        (&tags.artist, TagCategory::Artist),
        (&tags.contributor, TagCategory::Contributor),
        (&tags.copyright, TagCategory::Copyright),
        (&tags.character, TagCategory::Character),
        (&tags.species, TagCategory::Species),
        (&tags.invalid, TagCategory::Invalid),
        (&tags.meta, TagCategory::Meta),
        (&tags.lore, TagCategory::Lore),
    ]
    .into_iter()
    .find_map(|(list, category)| list.iter().any(|tag| tag == name).then_some(category))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use tokio::sync::mpsc;

    use super::*;
    use crate::{
        api::testing::{Received, json_response, ok, serve},
        network::UserAgent,
    };

    fn page(ids: impl IntoIterator<Item = u64>) -> Value {
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/posts_webm.json")).unwrap();
        let post = &fixture["posts"][0];
        let posts: Vec<Value> = ids
            .into_iter()
            .map(|id| {
                let mut post = post.clone();
                post["id"] = json!(id);
                post
            })
            .collect();
        json!({ "posts": posts })
    }

    /// An anonymous `/users/me` reply carrying `blacklist`.
    fn me(blacklist: &str) -> String {
        let mut me: Value =
            serde_json::from_str(include_str!("../tests/fixtures/users_me_anonymous.json"))
                .unwrap();
        me["blacklisted_tags"] = json!(blacklist);
        ok(&me)
    }

    /// The receiver must outlive the requests: the fake server reports each one to it.
    async fn api(responses: Vec<String>) -> (Api, mpsc::UnboundedReceiver<Received>) {
        let (base, received) = serve(responses).await;
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (Api::with_base(http, UserAgent::new(), base), received)
    }

    #[tokio::test(start_paused = true)]
    async fn reads_pages_until_done() {
        let (api, _received) = api(vec![me(""), ok(&page(1..=100)), ok(&page([0]))]).await;
        let searches = Searches::default();
        let id = searches.start("rabbit");
        let first = searches.next(&api, id).await.unwrap();
        assert_eq!((first.posts.len(), first.done), (100, false));
        let second = searches.next(&api, id).await.unwrap();
        assert_eq!((second.posts.len(), second.done), (1, true));
    }

    #[tokio::test]
    async fn keeps_one_search_per_screen() {
        let (api, _received) = api(Vec::new()).await;
        let searches = Searches::default();
        let favorites = searches.start_favorites();
        let first = searches.start("rabbit");
        searches.start("frog");
        assert_eq!(
            searches.next(&api, first).await,
            Err(SearchError::Closed(ClosedSearch::Closed))
        );
        assert!(searches.open.lock().unwrap().contains_key(&favorites));
    }

    #[tokio::test]
    async fn starting_a_search_closes_the_previous_one() {
        let (api, _received) = api(Vec::new()).await;
        let searches = Searches::default();
        let old = searches.start("rabbit");
        let new = searches.start("frog");
        assert_ne!(old, new);
        assert_eq!(
            searches.next(&api, old).await,
            Err(SearchError::Closed(ClosedSearch::Closed))
        );
        searches.close(new);
        assert_eq!(
            searches.next(&api, new).await,
            Err(SearchError::Closed(ClosedSearch::Closed))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn passes_api_errors_through() {
        let mut responses = vec![me("")];
        responses.extend(vec![json_response("503 Service Unavailable", "{}"); 4]);
        let (api, _received) = api(responses).await;
        let searches = Searches::default();
        let id = searches.start("rabbit");
        assert_eq!(
            searches.next(&api, id).await,
            Err(SearchError::Api(ApiError::Unavailable))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn finds_the_categories_of_searched_tags_in_the_posts() {
        let (api, _received) = api(vec![me(""), ok(&page([1]))]).await;
        let searches = Searches::default();
        let id = searches.start("Rabbit -natural_habitok ~frog rating:s yoga* unknown_tag");
        let page = searches.next(&api, id).await.unwrap();
        assert_eq!(
            page.categories,
            HashMap::from([
                ("rabbit".to_owned(), TagCategory::Species),
                ("natural_habitok".to_owned(), TagCategory::Artist),
                ("frog".to_owned(), TagCategory::Species),
            ])
        );
    }

    #[tokio::test(start_paused = true)]
    async fn marks_blacklisted_posts() {
        let (api, _received) = api(vec![me("id:<50"), ok(&page(1..=100))]).await;
        let searches = Searches::default();
        let id = searches.start("rabbit");
        let page = searches.next(&api, id).await.unwrap();
        assert_eq!(page.posts.len(), 100);
        assert_eq!(page.blacklisted, (1..50).collect::<Vec<u64>>());
    }

    #[tokio::test(start_paused = true)]
    async fn follows_fully_blacklisted_pages_up_to_a_limit() {
        let mut responses = vec![me("rabbit")];
        responses.extend((0..6).map(|n| ok(&page(n * 100 + 1..=n * 100 + 100))));
        let (api, mut received) = api(responses).await;
        let searches = Searches::default();
        let id = searches.start("frog");
        let page = searches.next(&api, id).await.unwrap();
        assert_eq!(page.posts.len(), 500);
        assert_eq!(page.blacklisted.len(), 500);
        assert!(!page.done);
        let mut requests = 0;
        while received.try_recv().is_ok() {
            requests += 1;
        }
        assert_eq!(requests, 1 + MAX_PAGES_PER_CALL);
    }

    #[tokio::test(start_paused = true)]
    async fn returns_nothing_until_the_blacklist_is_known() {
        let (api, mut received) =
            api(vec![json_response("503 Service Unavailable", "{}"); 4]).await;
        let searches = Searches::default();
        let id = searches.start("frog");
        assert_eq!(
            searches.next(&api, id).await,
            Err(SearchError::Api(ApiError::Unavailable))
        );
        while let Ok(request) = received.try_recv() {
            assert_eq!(request.target, "/users/me.json");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn fetches_the_blacklist_again_for_new_credentials() {
        let (api, mut received) =
            api(vec![me(""), ok(&page([1])), me("rabbit"), ok(&page([2]))]).await;
        let searches = Searches::default();
        let first = searches.start("frog");
        assert!(
            searches
                .next(&api, first)
                .await
                .unwrap()
                .blacklisted
                .is_empty()
        );
        api.set_credentials(Some(Arc::new(Credentials {
            username: "alice".into(),
            api_key: "key".into(),
        })));
        let second = searches.start("frog");
        assert_eq!(searches.next(&api, second).await.unwrap().blacklisted, [2]);
        let targets: Vec<String> = std::iter::from_fn(|| received.try_recv().ok())
            .map(|request| request.target)
            .collect();
        assert_eq!(
            targets
                .iter()
                .filter(|target| *target == "/users/me.json")
                .count(),
            2
        );
    }

    #[tokio::test(start_paused = true)]
    async fn refreshes_the_blacklist_after_a_while_and_marks_loaded_posts_anew() {
        let (api, mut received) = api(vec![
            me(""),
            ok(&page(1..=100)),
            me("id:2\nid:150"),
            ok(&page(101..=200)),
        ])
        .await;
        let searches = Searches::default();
        let id = searches.start("rabbit");
        let first = searches.next(&api, id).await.unwrap();
        assert!(first.blacklisted.is_empty());
        assert_eq!(searches.refresh_blacklist(&api).await, Ok(None));

        tokio::time::advance(BLACKLIST_REFRESH).await;
        let change = searches.refresh_blacklist(&api).await.unwrap().unwrap();
        assert!(change.generation > first.blacklist_generation);
        assert_eq!(
            change.searches,
            [SearchBlacklisted {
                id,
                blacklisted: vec![2],
            }]
        );
        let second = searches.next(&api, id).await.unwrap();
        assert_eq!(second.blacklisted, [150]);
        assert_eq!(second.blacklist_generation, change.generation);

        let me_requests = std::iter::from_fn(|| received.try_recv().ok())
            .filter(|request| request.target == "/users/me.json")
            .count();
        assert_eq!(me_requests, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn an_unchanged_blacklist_changes_nothing() {
        let (api, _received) = api(vec![me("id:1"), ok(&page(1..=100)), me("id:1")]).await;
        let searches = Searches::default();
        let id = searches.start("rabbit");
        searches.next(&api, id).await.unwrap();
        tokio::time::advance(BLACKLIST_REFRESH).await;
        assert_eq!(searches.refresh_blacklist(&api).await, Ok(None));
    }

    #[tokio::test(start_paused = true)]
    async fn nothing_is_refreshed_before_the_first_page() {
        let (api, mut received) = api(Vec::new()).await;
        let searches = Searches::default();
        tokio::time::advance(BLACKLIST_REFRESH).await;
        assert_eq!(searches.refresh_blacklist(&api).await, Ok(None));
        assert!(received.try_recv().is_err());
    }
}
