//! The tag searches the UI has open, each read page by page.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::Serialize;
use tokio::sync::Mutex;

use crate::api::{
    Api, ApiError, PostSearch,
    models::{Post, TagCategory, Tags},
};

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub posts: Vec<Post>,
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

struct OpenSearch {
    tags: Vec<String>,
    pages: Mutex<PostSearch>,
}

#[derive(Default)]
pub struct Searches {
    next_id: AtomicU64,
    open: std::sync::Mutex<HashMap<u64, Arc<OpenSearch>>>,
}

impl Searches {
    /// The UI shows one search at a time, so starting one closes the others.
    pub fn start(&self, tags: &str) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let search = Arc::new(OpenSearch {
            tags: tags.split_whitespace().map(str::to_owned).collect(),
            pages: Mutex::new(PostSearch::new(tags)),
        });
        let mut open = self.open.lock().expect("searches lock poisoned");
        open.clear();
        open.insert(id, search);
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
        let mut pages = search.pages.lock().await;
        let posts = pages.next_page(api).await.map_err(SearchError::Api)?;
        Ok(SearchPage {
            categories: categories(&search.tags, &posts),
            done: pages.is_done(),
            posts,
        })
    }

    pub fn close(&self, id: u64) {
        self.open
            .lock()
            .expect("searches lock poisoned")
            .remove(&id);
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

    /// The receiver must outlive the requests: the fake server reports each one to it.
    async fn api(responses: Vec<String>) -> (Api, mpsc::UnboundedReceiver<Received>) {
        let (base, received) = serve(responses).await;
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (Api::with_base(http, UserAgent::new(), base), received)
    }

    #[tokio::test(start_paused = true)]
    async fn reads_pages_until_done() {
        let (api, _received) = api(vec![ok(&page(1..=100)), ok(&page([0]))]).await;
        let searches = Searches::default();
        let id = searches.start("rabbit");
        let first = searches.next(&api, id).await.unwrap();
        assert_eq!((first.posts.len(), first.done), (100, false));
        let second = searches.next(&api, id).await.unwrap();
        assert_eq!((second.posts.len(), second.done), (1, true));
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
        let (api, _received) = api(vec![json_response("503 Service Unavailable", "{}"); 4]).await;
        let searches = Searches::default();
        let id = searches.start("rabbit");
        assert_eq!(
            searches.next(&api, id).await,
            Err(SearchError::Api(ApiError::Unavailable))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn finds_the_categories_of_searched_tags_in_the_posts() {
        let (api, _received) = api(vec![ok(&page([1]))]).await;
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
}
