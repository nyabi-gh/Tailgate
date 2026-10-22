use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use super::{
    Api, ApiError, Credentials,
    models::{
        Favorited, Pool, PoolCategory, Post, RawPool, RawPostEnvelope, RawPosts, RawUser,
        TagSuggestion, User, Voted,
    },
};

pub const PAGE_SIZE: usize = 100;
/// e621's own page size for pool lists.
pub const POOL_PAGE_SIZE: usize = 75;
/// `page=<number>` answers 410 past this page; only `page=b<id>` goes deeper.
const MAX_PAGE_NUMBER: u32 = 750;

impl Api {
    pub async fn post(&self, id: u64) -> Result<Post, ApiError> {
        let raw: RawPostEnvelope = self.client.get(&format!("posts/{id}.json"), &[]).await?;
        Ok(raw.post.into())
    }

    /// `None` when the request is anonymous.
    pub async fn me(&self) -> Result<Option<User>, ApiError> {
        let raw: RawUser = self.client.get("users/me.json", &[]).await?;
        Ok(raw.into_user())
    }

    /// Checks credentials without making them the current ones: `None` if e621 does not
    /// accept them as a user.
    pub async fn me_as(&self, credentials: &Credentials) -> Result<Option<User>, ApiError> {
        let raw: RawUser = self
            .client
            .get_as("users/me.json", &[], Some(credentials))
            .await?;
        Ok(raw.into_user())
    }

    /// The blacklist e621 applies to the current credentials: the user's own, or the site's
    /// default for anonymous requests.
    pub async fn blacklist(&self) -> Result<String, ApiError> {
        let raw: RawUser = self.client.get("users/me.json", &[]).await?;
        Ok(raw.into_blacklist())
    }

    pub async fn autocomplete(&self, text: &str) -> Result<Vec<TagSuggestion>, ApiError> {
        self.client
            .get("tags/autocomplete.json", &[("search[name_matches]", text)])
            .await
    }

    pub async fn favorite(&self, id: u64) -> Result<Favorited, ApiError> {
        self.client
            .post("favorites.json", &[("post_id", &id.to_string())])
            .await
    }

    pub async fn unfavorite(&self, id: u64) -> Result<Favorited, ApiError> {
        self.client.delete(&format!("favorites/{id}.json")).await
    }

    /// Voting the same way again takes the vote back, as on the site; the reply says where it
    /// ended up.
    pub async fn vote(&self, id: u64, up: bool) -> Result<Voted, ApiError> {
        let score = if up { "1" } else { "-1" };
        self.client
            .post(&format!("posts/{id}/votes.json"), &[("score", score)])
            .await
    }

    /// The signed-in user's favorites, most recently added first.
    async fn favorites(&self, page: u32) -> Result<Vec<Post>, ApiError> {
        let (limit, page) = (PAGE_SIZE.to_string(), page.to_string());
        let raw: RawPosts = self
            .client
            .get("favorites.json", &[("limit", &limit), ("page", &page)])
            .await?;
        Ok(raw.posts.into_iter().map(Post::from).collect())
    }

    pub async fn pools(&self, query: &PoolQuery, page: u32) -> Result<Vec<Pool>, ApiError> {
        let (limit, page) = (POOL_PAGE_SIZE.to_string(), page.to_string());
        let mut params = vec![
            ("search[order]", query.order.as_param()),
            ("limit", limit.as_str()),
            ("page", page.as_str()),
        ];
        let name = query.name.trim();
        if !name.is_empty() {
            params.push(("search[name_matches]", name));
        }
        if let Some(category) = query.category {
            params.push((
                "search[category]",
                match category {
                    PoolCategory::Series => "series",
                    PoolCategory::Collection => "collection",
                },
            ));
        }
        let raw: Vec<RawPool> = self.client.get("pools.json", &params).await?;
        Ok(raw.into_iter().map(Pool::from).collect())
    }

    pub async fn pool(&self, id: u64) -> Result<Pool, ApiError> {
        let raw: RawPool = self.client.get(&format!("pools/{id}.json"), &[]).await?;
        Ok(raw.into())
    }

    /// In no particular order.
    pub async fn pools_by_id(&self, ids: &[u64]) -> Result<Vec<Pool>, ApiError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = join_ids(ids);
        let raw: Vec<RawPool> = self
            .client
            .get("pools.json", &[("search[id]", &ids)])
            .await?;
        Ok(raw.into_iter().map(Pool::from).collect())
    }

    /// In the order of `ids`. Posts e621 leaves out, such as deleted ones, are missing.
    pub async fn posts_by_id(&self, ids: &[u64]) -> Result<Vec<Post>, ApiError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let (tags, limit) = (format!("id:{}", join_ids(ids)), ids.len().to_string());
        let raw: RawPosts = self
            .client
            .get("posts.json", &[("tags", &tags), ("limit", &limit)])
            .await?;
        // e621 answers newest first, whatever the order asked for.
        let mut found: HashMap<u64, Post> = raw
            .posts
            .into_iter()
            .map(|post| {
                let post = Post::from(post);
                (post.id, post)
            })
            .collect();
        Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
    }

    async fn popular(&self, popular: &Popular) -> Result<Vec<Post>, ApiError> {
        let raw: RawPosts = self
            .client
            .get(
                "popular.json",
                &[
                    ("date", &popular.date),
                    ("scale", popular.scale.as_str()),
                    ("time_zone", &popular.time_zone),
                ],
            )
            .await?;
        Ok(raw.posts.into_iter().map(Post::from).collect())
    }

    async fn posts(&self, tags: &str, page: Option<&str>) -> Result<Vec<Post>, ApiError> {
        let limit = PAGE_SIZE.to_string();
        let mut query = vec![("tags", tags), ("limit", limit.as_str())];
        if let Some(page) = page {
            query.push(("page", page));
        }
        let raw: RawPosts = self.client.get("posts.json", &query).await?;
        Ok(raw.posts.into_iter().map(Post::from).collect())
    }
}

fn join_ids(ids: &[u64]) -> String {
    ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",")
}

/// What the pool list shows.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolQuery {
    /// Part of the name; empty for all pools.
    pub name: String,
    /// `None` for both.
    pub category: Option<PoolCategory>,
    pub order: PoolOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PoolOrder {
    /// Recently changed first.
    Updated,
    Name,
    /// Newest first.
    Created,
    /// Largest first.
    PostCount,
}

impl PoolOrder {
    fn as_param(self) -> &'static str {
        match self {
            Self::Updated => "updated_at",
            Self::Name => "name",
            Self::Created => "created_at",
            Self::PostCount => "post_count",
        }
    }
}

/// One period of e621's popular posts: the highest scored posts made in it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Popular {
    /// `YYYY-MM-DD`, any day of the period.
    pub date: String,
    pub scale: Scale,
    /// The zone `date` is read in; e621 otherwise reads it in the account's zone, by default
    /// US Eastern.
    pub time_zone: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scale {
    Day,
    /// Monday to Sunday.
    Week,
    Month,
}

impl Scale {
    fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }
}

/// One tag search, the user's favorites, or a period's popular posts, read page by page. Keeps
/// the ids it has returned so a post that moves to a later page while paging by number is not
/// shown twice.
pub struct PostSearch {
    source: Source,
    next: Option<Page>,
    seen: HashSet<u64>,
}

enum Source {
    Tags(String),
    Favorites,
    /// A single page: e621 offers no more.
    Popular(Popular),
    /// A pool's or a set's posts, in its order, [`PAGE_SIZE`] ids per page.
    Ids(Vec<u64>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Page {
    First,
    /// Posts older than this id. Has no depth limit but forces `order:id_desc`, so it is only
    /// used when the search does not ask for another order.
    Before(u64),
    Number(u32),
}

impl PostSearch {
    pub fn new(tags: &str) -> Self {
        let tags = tags.split_whitespace().collect::<Vec<_>>().join(" ");
        let ordered = tags
            .split(' ')
            .any(|tag| tag.to_ascii_lowercase().starts_with("order:"));
        Self {
            next: Some(if ordered {
                Page::Number(1)
            } else {
                Page::First
            }),
            source: Source::Tags(tags),
            seen: HashSet::new(),
        }
    }

    /// Favorites only page by number, in the order they were added.
    pub fn favorites() -> Self {
        Self {
            source: Source::Favorites,
            next: Some(Page::Number(1)),
            seen: HashSet::new(),
        }
    }

    /// Pages by number, each the next [`PAGE_SIZE`] of `ids`.
    pub fn ids(ids: Vec<u64>) -> Self {
        Self {
            next: (!ids.is_empty()).then_some(Page::Number(1)),
            source: Source::Ids(ids),
            seen: HashSet::new(),
        }
    }

    pub fn popular(popular: Popular) -> Self {
        Self {
            source: Source::Popular(popular),
            next: Some(Page::First),
            seen: HashSet::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn skip_to_page(&mut self, number: u32) {
        self.next = Some(Page::Number(number));
    }

    pub fn is_done(&self) -> bool {
        self.next.is_none()
    }

    /// The next page's posts not returned before; empty once the search is done. After an
    /// error the same page is asked for again on the next call.
    pub async fn next_page(&mut self, api: &Api) -> Result<Vec<Post>, ApiError> {
        let Some(page) = self.next else {
            return Ok(Vec::new());
        };
        let posts = match (&self.source, page) {
            (Source::Favorites, Page::Number(number)) => api.favorites(number).await?,
            (Source::Favorites, _) => unreachable!("favorites page by number"),
            (Source::Popular(popular), _) => api.popular(popular).await?,
            (Source::Ids(ids), Page::Number(number)) => {
                api.posts_by_id(ids_page(ids, number)).await?
            }
            (Source::Ids(_), _) => unreachable!("id lists page by number"),
            (Source::Tags(tags), page) => {
                let param = match page {
                    Page::First => None,
                    Page::Before(id) => Some(format!("b{id}")),
                    Page::Number(number) => Some(number.to_string()),
                };
                api.posts(tags, param.as_deref()).await?
            }
        };
        self.next = match &self.source {
            Source::Popular(_) => None,
            Source::Ids(ids) => match page {
                Page::Number(number) if (number as usize) * PAGE_SIZE < ids.len() => {
                    Some(Page::Number(number + 1))
                }
                _ => None,
            },
            Source::Tags(_) | Source::Favorites => following(page, &posts),
        };
        Ok(posts
            .into_iter()
            .filter(|post| self.seen.insert(post.id))
            .collect())
    }
}

/// Page `number`, counted from 1, of `ids`.
fn ids_page(ids: &[u64], number: u32) -> &[u64] {
    let start = (number as usize - 1) * PAGE_SIZE;
    &ids[start.min(ids.len())..(start + PAGE_SIZE).min(ids.len())]
}

fn following(page: Page, posts: &[Post]) -> Option<Page> {
    if posts.len() < PAGE_SIZE {
        return None;
    }
    match page {
        Page::Number(number) if number >= MAX_PAGE_NUMBER => None,
        Page::Number(number) => Some(Page::Number(number + 1)),
        Page::First | Page::Before(_) => posts.iter().map(|post| post.id).min().map(Page::Before),
    }
}
