use std::collections::HashSet;

use super::{
    Api, ApiError, Credentials,
    models::{Favorited, Post, RawPostEnvelope, RawPosts, RawUser, TagSuggestion, User, Voted},
};

pub const PAGE_SIZE: usize = 100;
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

/// One tag search, or the user's favorites, read page by page. Keeps the ids it has returned so
/// a post that moves to a later page while paging by number is not shown twice.
pub struct PostSearch {
    /// `None` for the favorites.
    tags: Option<String>,
    next: Option<Page>,
    seen: HashSet<u64>,
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
            tags: Some(tags),
            seen: HashSet::new(),
        }
    }

    /// Favorites only page by number, in the order they were added.
    pub fn favorites() -> Self {
        Self {
            tags: None,
            next: Some(Page::Number(1)),
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
        let posts = match (&self.tags, page) {
            (None, Page::Number(number)) => api.favorites(number).await?,
            (None, _) => unreachable!("favorites page by number"),
            (Some(tags), page) => {
                let param = match page {
                    Page::First => None,
                    Page::Before(id) => Some(format!("b{id}")),
                    Page::Number(number) => Some(number.to_string()),
                };
                api.posts(tags, param.as_deref()).await?
            }
        };
        self.next = following(page, &posts);
        Ok(posts
            .into_iter()
            .filter(|post| self.seen.insert(post.id))
            .collect())
    }
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
