//! The e621 API. Only Rust calls it: a request from the webview would carry a browser
//! User-Agent and bypass the throttle (docs/report.md §1).

mod client;
mod endpoints;
pub mod models;
#[cfg(test)]
pub(crate) mod testing;

use std::sync::Arc;

use serde::Serialize;
use tokio::sync::watch;

use crate::network::UserAgent;
use client::Client;
pub use client::Credentials;
pub use endpoints::{
    LIST_PAGE_SIZE, PAGE_SIZE, PoolOrder, PoolQuery, Popular, PostSearch, Scale, SetOrder, SetQuery,
};

const BASE: &str = "https://e621.net/";

/// Why a request failed. The UI words it; the details only go to the log.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum ApiError {
    #[error("the credentials were rejected")]
    Unauthorized,
    #[error("access denied")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("past the last page reachable by number")]
    PageLimit,
    #[error("rate limited")]
    RateLimited,
    #[error("e621 is unavailable")]
    Unavailable,
    #[error("could not reach e621")]
    Network,
    #[error("unexpected response")]
    Decode,
    #[error("unexpected status {status}")]
    Http { status: u16 },
}

impl ApiError {
    fn from_status(status: reqwest::StatusCode) -> Self {
        match status.as_u16() {
            401 => Self::Unauthorized,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            410 => Self::PageLimit,
            429 => Self::RateLimited,
            503 => Self::Unavailable,
            status => Self::Http { status },
        }
    }
}

pub struct Api {
    client: Client,
}

impl Api {
    /// `http` should be the app's e621 client, so API requests take the same route and kill
    /// switch as everything else.
    /// `user_agent` follows the credentials set here, for requests outside the API (media).
    pub fn new(http: reqwest::Client, user_agent: UserAgent) -> Self {
        Self::with_base(http, user_agent, BASE.to_owned())
    }

    pub(crate) fn with_base(http: reqwest::Client, user_agent: UserAgent, base: String) -> Self {
        Self {
            client: Client::new(http, base, user_agent),
        }
    }

    pub fn credentials(&self) -> Option<Arc<Credentials>> {
        self.client.credentials()
    }

    pub fn set_credentials(&self, credentials: Option<Arc<Credentials>>) {
        self.client.set_credentials(credentials);
    }

    /// Credentials e621 rejected (401) while they were in use: an API key that expired or was
    /// revoked.
    pub fn watch_rejections(&self) -> watch::Receiver<Option<Arc<Credentials>>> {
        self.client.watch_rejections()
    }
}

#[cfg(test)]
mod tests {
    use std::{net::Ipv4Addr, time::Duration};

    use serde_json::{Value, json};
    use tokio::{net::TcpListener, sync::mpsc};

    use super::{
        models::{PoolCategory, Post},
        testing::{Received, json_response, ok, serve},
        *,
    };
    use crate::network::USER_AGENT;

    async fn fake_api(responses: Vec<String>) -> (Api, mpsc::UnboundedReceiver<Received>) {
        let (base, received) = serve(responses).await;
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (Api::with_base(http, UserAgent::new(), base), received)
    }

    fn anonymous() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/users_me_anonymous.json")).unwrap()
    }

    /// `ids.len()` copies of a fixture post with the given ids.
    fn posts(ids: impl IntoIterator<Item = u64>) -> Value {
        let fixture: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/posts_image.json")).unwrap();
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

    fn drain(received: &mut mpsc::UnboundedReceiver<Received>) -> Vec<Received> {
        let mut all = Vec::new();
        while let Ok(request) = received.try_recv() {
            all.push(request);
        }
        all
    }

    #[tokio::test(start_paused = true)]
    async fn identifies_the_user_in_the_user_agent_and_authorization() {
        let (api, mut received) = fake_api(vec![ok(&anonymous()), ok(&anonymous())]).await;
        assert_eq!(api.me().await, Ok(None));
        api.set_credentials(Some(Arc::new(Credentials {
            username: "alice".into(),
            api_key: "key".into(),
        })));
        api.me().await.unwrap();
        let requests = drain(&mut received);
        assert_eq!(requests[0].target, "/users/me.json");
        assert_eq!(requests[0].user_agent, USER_AGENT);
        assert_eq!(requests[0].authorization, None);
        assert_eq!(
            requests[1].user_agent,
            format!("Tailgate/{} (by alice on e621)", env!("CARGO_PKG_VERSION"))
        );
        // base64("alice:key")
        assert_eq!(
            requests[1].authorization.as_deref(),
            Some("Basic YWxpY2U6a2V5")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn retries_after_the_delay_the_server_asks_for() {
        let busy = "HTTP/1.1 503 Service Unavailable\r\nretry-after: 3\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
        let (api, mut received) = fake_api(vec![busy.into(), ok(&anonymous())]).await;
        assert_eq!(api.me().await, Ok(None));
        let requests = drain(&mut received);
        assert_eq!(requests.len(), 2);
        assert!(requests[1].at - requests[0].at >= Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_three_retries() {
        let limited = json_response("429 Too Many Requests", "{}");
        let (api, mut received) = fake_api(vec![limited; 4]).await;
        assert_eq!(api.me().await, Err(ApiError::RateLimited));
        assert_eq!(drain(&mut received).len(), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn does_not_wait_out_long_retry_after_delays() {
        let busy = "HTTP/1.1 503 Service Unavailable\r\nretry-after: 600\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
        let (api, mut received) = fake_api(vec![busy.into()]).await;
        assert_eq!(api.me().await, Err(ApiError::Unavailable));
        assert_eq!(drain(&mut received).len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn reports_failures_as_codes() {
        let (api, _received) = fake_api(vec![
            json_response("401 Unauthorized", "{}"),
            json_response("410 Gone", "{}"),
            json_response("418 I'm a teapot", "{}"),
            json_response("200 OK", "not json"),
        ])
        .await;
        assert_eq!(api.me().await, Err(ApiError::Unauthorized));
        assert_eq!(api.me().await, Err(ApiError::PageLimit));
        assert_eq!(api.me().await, Err(ApiError::Http { status: 418 }));
        assert_eq!(api.me().await, Err(ApiError::Decode));
    }

    #[tokio::test]
    async fn reports_unreachable_servers_as_network_errors() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        drop(listener);
        let api = Api::with_base(
            reqwest::Client::builder().no_proxy().build().unwrap(),
            UserAgent::new(),
            base,
        );
        assert_eq!(api.me().await, Err(ApiError::Network));
    }

    #[tokio::test(start_paused = true)]
    async fn pages_by_the_lowest_id_and_skips_posts_seen_before() {
        let first: Vec<u64> = (101..=200).rev().collect();
        let (api, mut received) =
            fake_api(vec![ok(&posts(first)), ok(&posts([101, 100, 99]))]).await;
        let mut search = PostSearch::new("  wolf   solo ");
        assert_eq!(search.next_page(&api).await.unwrap().len(), PAGE_SIZE);
        assert!(!search.is_done());
        let ids: Vec<u64> = search
            .next_page(&api)
            .await
            .unwrap()
            .iter()
            .map(|post| post.id)
            .collect();
        assert_eq!(ids, [100, 99]);
        assert!(search.is_done());
        assert!(search.next_page(&api).await.unwrap().is_empty());

        let requests = drain(&mut received);
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].target, "/posts.json?tags=wolf+solo&limit=100");
        assert_eq!(
            requests[1].target,
            "/posts.json?tags=wolf+solo&limit=100&page=b101"
        );
    }

    /// The search screen opens on this: e621's own front page, the newest posts.
    #[tokio::test(start_paused = true)]
    async fn an_empty_search_lists_the_newest_posts_by_cursor() {
        let (api, mut received) =
            fake_api(vec![ok(&posts((101..=200).rev())), ok(&posts([100]))]).await;
        let mut search = PostSearch::new("");
        search.next_page(&api).await.unwrap();
        search.next_page(&api).await.unwrap();
        let requests = drain(&mut received);
        assert_eq!(requests[0].target, "/posts.json?tags=&limit=100");
        assert_eq!(requests[1].target, "/posts.json?tags=&limit=100&page=b101");
    }

    #[tokio::test(start_paused = true)]
    async fn pages_by_number_when_the_search_sets_an_order() {
        let (api, mut received) = fake_api(vec![ok(&posts(1..=100)), ok(&posts(101..=150))]).await;
        let mut search = PostSearch::new("wolf Order:score");
        search.next_page(&api).await.unwrap();
        search.next_page(&api).await.unwrap();
        assert!(search.is_done());
        let requests = drain(&mut received);
        assert_eq!(
            requests[0].target,
            "/posts.json?tags=wolf+Order%3Ascore&limit=100&page=1"
        );
        assert_eq!(
            requests[1].target,
            "/posts.json?tags=wolf+Order%3Ascore&limit=100&page=2"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn popular_posts_are_one_page_for_the_asked_period() {
        let (api, mut received) = fake_api(vec![ok(&posts((1..=75).rev()))]).await;
        // As the UI sends it.
        let popular: Popular = serde_json::from_value(json!({
            "date": "2026-10-22",
            "scale": "week",
            "timeZone": "Asia/Seoul",
        }))
        .unwrap();
        let mut search = PostSearch::popular(popular);
        assert_eq!(search.next_page(&api).await.unwrap().len(), 75);
        assert!(search.is_done());
        assert!(search.next_page(&api).await.unwrap().is_empty());
        let requests = drain(&mut received);
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].target,
            "/popular.json?date=2026-10-22&scale=week&time_zone=Asia%2FSeoul"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn reads_an_id_list_in_its_own_order_page_by_page() {
        // e621 answers newest first and leaves out deleted posts, here 2.
        let first: Vec<u64> = (1..=100).rev().filter(|id| *id != 2).collect();
        let (api, mut received) =
            fake_api(vec![ok(&posts(first)), ok(&posts((101..=150).rev()))]).await;
        let mut search = PostSearch::ids((1..=150).collect());
        let ids = |posts: Vec<_>| posts.iter().map(|post: &Post| post.id).collect::<Vec<_>>();

        let page = ids(search.next_page(&api).await.unwrap());
        assert_eq!(page.len(), 99);
        assert_eq!(page[..3], [1, 3, 4]);
        assert!(!search.is_done());
        let page = ids(search.next_page(&api).await.unwrap());
        assert_eq!(page, (101..=150).collect::<Vec<_>>());
        assert!(search.is_done());

        let requests = drain(&mut received);
        assert!(
            requests[0]
                .target
                .starts_with("/posts.json?tags=id%3A1%2C2%2C3%2C")
        );
        assert!(requests[0].target.ends_with("%2C100&limit=100"));
        assert!(
            requests[1]
                .target
                .starts_with("/posts.json?tags=id%3A101%2C")
        );
        assert!(requests[1].target.ends_with("%2C150&limit=50"));
    }

    #[tokio::test]
    async fn an_empty_id_list_asks_for_nothing() {
        let (api, mut received) = fake_api(Vec::new()).await;
        let mut search = PostSearch::ids(Vec::new());
        assert!(search.is_done());
        assert!(search.next_page(&api).await.unwrap().is_empty());
        assert!(api.posts_by_id(&[]).await.unwrap().is_empty());
        assert!(drain(&mut received).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn asks_for_pools_by_name_category_and_order() {
        let (api, mut received) = fake_api(vec![ok(&json!([])), ok(&json!([]))]).await;
        let query = PoolQuery {
            name: " wolf pack ".into(),
            category: Some(PoolCategory::Series),
            order: PoolOrder::PostCount,
        };
        api.pools(&query, 2).await.unwrap();
        let all = PoolQuery {
            name: String::new(),
            category: None,
            order: PoolOrder::Updated,
        };
        api.pools(&all, 1).await.unwrap();
        let requests = drain(&mut received);
        assert_eq!(
            requests[0].target,
            "/pools.json?search%5Border%5D=post_count&limit=75&page=2&search%5Bname_matches%5D=wolf+pack&search%5Bcategory%5D=series"
        );
        assert_eq!(
            requests[1].target,
            "/pools.json?search%5Border%5D=updated_at&limit=75&page=1"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn asks_for_sets_by_name_and_order() {
        let (api, mut received) = fake_api(vec![ok(&json!([])), ok(&json!([]))]).await;
        let query = SetQuery {
            name: "wolf".into(),
            order: SetOrder::PostCount,
        };
        api.sets(&query, 3).await.unwrap();
        let newest = SetQuery {
            name: String::new(),
            order: SetOrder::Created,
        };
        api.sets(&newest, 1).await.unwrap();
        let requests = drain(&mut received);
        assert_eq!(
            requests[0].target,
            "/post_sets.json?limit=75&page=3&search%5Border%5D=post_count&search%5Bname%5D=wolf"
        );
        // e621's own default is newest first; its `created_at` order is the opposite.
        assert_eq!(requests[1].target, "/post_sets.json?limit=75&page=1");
    }

    #[tokio::test(start_paused = true)]
    async fn adds_to_and_removes_from_sets() {
        let set = json!({"id": 5, "name": "n", "shortname": "s", "is_public": true,
            "updated_at": "2026-10-22T00:00:00.000-04:00", "post_ids": [7]});
        let (api, mut received) = fake_api(vec![
            json_response("201 Created", &set.to_string()),
            ok(&set),
        ])
        .await;
        api.change_set(5, 7, true).await.unwrap();
        api.change_set(5, 7, false).await.unwrap();
        let requests = drain(&mut received);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].target, "/post_sets/5/add_posts.json");
        assert_eq!(requests[0].body, "post_ids%5B%5D=7");
        assert_eq!(requests[1].target, "/post_sets/5/remove_posts.json");
    }

    #[tokio::test(start_paused = true)]
    async fn reads_the_ids_of_my_sets() {
        let (api, _received) = fake_api(vec![ok(&json!({
            "Owned": [["mine one", 1], ["mine two", 2]],
            "Maintained": [["theirs", 9]],
        }))])
        .await;
        assert_eq!(api.my_set_ids().await.unwrap(), (vec![1, 2], vec![9]));
    }

    #[tokio::test(start_paused = true)]
    async fn stops_at_the_last_page_reachable_by_number() {
        let (api, mut received) = fake_api(vec![ok(&posts(1..=100))]).await;
        let mut search = PostSearch::new("order:random");
        search.skip_to_page(750);
        search.next_page(&api).await.unwrap();
        assert!(search.is_done());
        assert!(drain(&mut received)[0].target.ends_with("&page=750"));
    }

    #[tokio::test(start_paused = true)]
    async fn asks_for_the_same_page_again_after_an_error() {
        let (api, mut received) = fake_api(vec![
            json_response("500 Internal Server Error", "{}"),
            ok(&posts([5])),
        ])
        .await;
        let mut search = PostSearch::new("wolf");
        assert!(search.next_page(&api).await.is_err());
        assert_eq!(search.next_page(&api).await.unwrap()[0].id, 5);
        let requests = drain(&mut received);
        assert_eq!(requests[0].target, requests[1].target);
    }

    #[tokio::test(start_paused = true)]
    async fn sends_favorites_and_votes() {
        let (api, mut received) = fake_api(vec![
            ok(&json!({ "post_id": 7, "favorite_count": 3 })),
            ok(&json!({ "post_id": 7, "favorite_count": 2 })),
            ok(&json!({ "score": 5, "up": 6, "down": -1, "our_score": 1 })),
            ok(&json!({ "score": 4, "up": 5, "down": -1, "our_score": 0 })),
        ])
        .await;
        assert_eq!(api.favorite(7).await.unwrap().favorite_count, 3);
        assert_eq!(api.unfavorite(7).await.unwrap().favorite_count, 2);
        assert_eq!(api.vote(7, true).await.unwrap().our_score, 1);
        assert_eq!(api.vote(7, true).await.unwrap().our_score, 0);
        let requests = drain(&mut received);
        let calls: Vec<(&str, &str, &str)> = requests
            .iter()
            .map(|request| {
                (
                    request.method.as_str(),
                    request.target.as_str(),
                    request.body.as_str(),
                )
            })
            .collect();
        assert_eq!(
            calls,
            [
                ("POST", "/favorites.json", "post_id=7"),
                ("DELETE", "/favorites/7.json", ""),
                ("POST", "/posts/7/votes.json", "score=1"),
                ("POST", "/posts/7/votes.json", "score=1"),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn pages_favorites_by_number() {
        let (api, mut received) = fake_api(vec![ok(&posts(1..=100)), ok(&posts([0]))]).await;
        let mut search = PostSearch::favorites();
        search.next_page(&api).await.unwrap();
        search.next_page(&api).await.unwrap();
        assert!(search.is_done());
        let targets: Vec<String> = drain(&mut received)
            .into_iter()
            .map(|request| request.target)
            .collect();
        assert_eq!(
            targets,
            [
                "/favorites.json?limit=100&page=1",
                "/favorites.json?limit=100&page=2"
            ]
        );
    }
}
