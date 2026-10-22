//! e621 responses as they arrive (`Raw*`, private) and the shapes the rest of the app uses.
//!
//! Field notes are in docs/research.md §2 and Pawru's docs/research.md §1.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum Rating {
    #[serde(rename = "s")]
    Safe,
    #[serde(rename = "q")]
    Questionable,
    #[serde(rename = "e")]
    Explicit,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Tags {
    #[serde(default)]
    pub general: Vec<String>,
    #[serde(default)]
    pub artist: Vec<String>,
    #[serde(default)]
    pub contributor: Vec<String>,
    #[serde(default)]
    pub copyright: Vec<String>,
    #[serde(default)]
    pub character: Vec<String>,
    #[serde(default)]
    pub species: Vec<String>,
    #[serde(default)]
    pub invalid: Vec<String>,
    #[serde(default)]
    pub meta: Vec<String>,
    #[serde(default)]
    pub lore: Vec<String>,
}

/// Tag categories by the numbers e621 uses for them.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(try_from = "u8", rename_all = "camelCase")]
pub enum TagCategory {
    General,
    Artist,
    Contributor,
    Copyright,
    Character,
    Species,
    Invalid,
    Meta,
    Lore,
}

impl TryFrom<u8> for TagCategory {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::General,
            1 => Self::Artist,
            2 => Self::Contributor,
            3 => Self::Copyright,
            4 => Self::Character,
            5 => Self::Species,
            6 => Self::Invalid,
            7 => Self::Meta,
            8 => Self::Lore,
            other => return Err(format!("unknown tag category {other}")),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct Score {
    pub up: i64,
    pub down: i64,
    pub total: i64,
}

/// A post as the app uses it. Media URLs are `None` for deleted posts and for posts hidden from
/// the current user (logged out, or safe mode); the rest of the metadata still arrives.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: u64,
    pub created_at: String,
    pub rating: Rating,
    pub score: Score,
    pub fav_count: u64,
    pub is_favorited: bool,
    /// The current user's vote: -1, 0 or 1.
    pub vote: i8,
    pub comment_count: u64,
    pub deleted: bool,
    pub pending: bool,
    pub flagged: bool,
    pub description: String,
    pub sources: Vec<String>,
    pub uploader: Uploader,
    pub pools: Vec<u64>,
    pub parent_id: Option<u64>,
    pub children: Vec<u64>,
    /// Seconds, for animations and videos.
    pub duration: Option<f64>,
    pub tags: Tags,
    pub file: File,
    pub preview: Image,
    pub sample: Option<Image>,
    pub video: Option<Video>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Uploader {
    pub id: u64,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct File {
    pub ext: String,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub md5: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub url: Option<String>,
}

/// The encodings static1 has of a video post. The MP4s are H.264, which every webview plays;
/// the original may be a WebM.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Video {
    pub p720: Option<VideoSource>,
    pub p480: Option<VideoSource>,
    pub mp4: Option<VideoSource>,
    pub original: Option<VideoSource>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VideoSource {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub codec: Option<String>,
}

/// The logged-in user. `/users/me.json` answers 200 for anonymous requests too, with a null id.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: u64,
    pub name: String,
    /// One rule per line, as the site stores it.
    pub blacklisted_tags: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSuggestion {
    pub id: u64,
    pub name: String,
    #[serde(alias = "post_count")]
    pub post_count: u64,
    pub category: TagCategory,
    /// The alias the typed text matched, when it matched an alias rather than the tag.
    #[serde(alias = "antecedent_name")]
    pub antecedent_name: Option<String>,
}

/// The reply to adding or removing a favorite.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorited {
    #[serde(alias = "post_id")]
    pub post_id: u64,
    #[serde(alias = "favorite_count")]
    pub favorite_count: u64,
}

/// The reply to a vote: the post's score after it, and the user's own vote (-1, 0 or 1).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voted {
    pub score: i64,
    pub up: i64,
    pub down: i64,
    #[serde(alias = "our_score")]
    pub our_score: i8,
}

#[derive(Deserialize)]
pub(super) struct RawPosts {
    pub posts: Vec<RawPost>,
}

#[derive(Deserialize)]
pub(super) struct RawPostEnvelope {
    pub post: RawPost,
}

#[derive(Deserialize)]
pub(super) struct RawPost {
    id: u64,
    created_at: String,
    file: RawFile,
    preview: RawImage,
    sample: RawSample,
    score: Score,
    tags: Tags,
    flags: RawFlags,
    rating: Rating,
    fav_count: u64,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    pools: Vec<u64>,
    relationships: RawRelationships,
    uploader_id: u64,
    #[serde(default)]
    uploader_name: Option<String>,
    #[serde(default)]
    description: String,
    comment_count: u64,
    is_favorited: bool,
    vote: i8,
    duration: Option<f64>,
}

#[derive(Deserialize)]
struct RawFile {
    width: u32,
    height: u32,
    ext: String,
    size: u64,
    md5: String,
    url: Option<String>,
}

#[derive(Deserialize)]
struct RawImage {
    width: u32,
    height: u32,
    url: Option<String>,
}

#[derive(Deserialize)]
struct RawSample {
    has: bool,
    width: u32,
    height: u32,
    url: Option<String>,
    #[serde(default)]
    alternates: RawAlternates,
}

/// `{}` for posts without video encodings.
#[derive(Default, Deserialize)]
struct RawAlternates {
    original: Option<RawVideo>,
    #[serde(default)]
    variants: HashMap<String, RawVideo>,
    #[serde(default)]
    samples: HashMap<String, RawVideo>,
}

#[derive(Deserialize)]
struct RawVideo {
    url: Option<String>,
    width: u32,
    height: u32,
    size: u64,
    codec: Option<String>,
}

#[derive(Deserialize)]
struct RawFlags {
    pending: bool,
    flagged: bool,
    deleted: bool,
}

#[derive(Deserialize)]
struct RawRelationships {
    parent_id: Option<u64>,
    #[serde(default)]
    children: Vec<u64>,
}

#[derive(Deserialize)]
pub(super) struct RawUser {
    id: Option<u64>,
    name: String,
    blacklisted_tags: Option<String>,
}

impl From<RawPost> for Post {
    fn from(raw: RawPost) -> Self {
        let RawAlternates {
            original,
            mut variants,
            mut samples,
        } = raw.sample.alternates;
        let video = Video {
            p720: samples.remove("720p").and_then(RawVideo::source),
            p480: samples.remove("480p").and_then(RawVideo::source),
            mp4: variants.remove("mp4").and_then(RawVideo::source),
            original: original.and_then(RawVideo::source),
        };
        Self {
            id: raw.id,
            created_at: raw.created_at,
            rating: raw.rating,
            score: raw.score,
            fav_count: raw.fav_count,
            is_favorited: raw.is_favorited,
            vote: raw.vote,
            comment_count: raw.comment_count,
            deleted: raw.flags.deleted,
            pending: raw.flags.pending,
            flagged: raw.flags.flagged,
            description: raw.description,
            sources: raw.sources,
            uploader: Uploader {
                id: raw.uploader_id,
                name: raw.uploader_name,
            },
            pools: raw.pools,
            parent_id: raw.relationships.parent_id,
            children: raw.relationships.children,
            duration: raw.duration,
            tags: raw.tags,
            file: File {
                ext: raw.file.ext,
                width: raw.file.width,
                height: raw.file.height,
                size: raw.file.size,
                md5: raw.file.md5,
                url: raw.file.url,
            },
            preview: Image {
                width: raw.preview.width,
                height: raw.preview.height,
                url: raw.preview.url,
            },
            sample: raw.sample.has.then_some(Image {
                width: raw.sample.width,
                height: raw.sample.height,
                url: raw.sample.url,
            }),
            video: (video != Video::default()).then_some(video),
        }
    }
}

impl RawVideo {
    fn source(self) -> Option<VideoSource> {
        Some(VideoSource {
            url: self.url?,
            width: self.width,
            height: self.height,
            size: self.size,
            codec: self.codec,
        })
    }
}

impl RawUser {
    pub(super) fn into_blacklist(self) -> String {
        self.blacklisted_tags.unwrap_or_default()
    }

    pub(super) fn into_user(self) -> Option<User> {
        Some(User {
            id: self.id?,
            name: self.name,
            blacklisted_tags: self.blacklisted_tags.unwrap_or_default(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PoolCategory {
    /// Meant to be read in order, like a comic.
    Series,
    Collection,
}

/// A pool. Its posts are listed by id, in reading order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pool {
    pub id: u64,
    /// e621 stores spaces as `_`; this is the name as shown.
    pub name: String,
    pub category: PoolCategory,
    pub creator: Option<String>,
    pub updated_at: String,
    #[serde(skip)]
    pub post_ids: Vec<u64>,
}

#[derive(Deserialize)]
pub(super) struct RawPool {
    id: u64,
    name: String,
    category: PoolCategory,
    creator_name: Option<String>,
    updated_at: String,
    post_ids: Vec<u64>,
}

impl From<RawPool> for Pool {
    fn from(raw: RawPool) -> Self {
        Self {
            id: raw.id,
            name: raw.name.replace('_', " "),
            category: raw.category,
            creator: raw.creator_name,
            updated_at: raw.updated_at,
            post_ids: raw.post_ids,
        }
    }
}

/// Posts from a `/posts.json` response, for tests elsewhere that start from real responses.
#[cfg(test)]
pub(crate) fn decode_posts(json: &str) -> Vec<Post> {
    let raw: RawPosts = serde_json::from_str(json).unwrap();
    raw.posts.into_iter().map(Post::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn posts(fixture: &str) -> Vec<Post> {
        let raw: RawPosts = serde_json::from_str(fixture).unwrap();
        raw.posts.into_iter().map(Post::from).collect()
    }

    #[test]
    fn decodes_pools() {
        let raw: Vec<RawPool> =
            serde_json::from_str(include_str!("../../tests/fixtures/pools.json")).unwrap();
        let pools: Vec<Pool> = raw.into_iter().map(Pool::from).collect();
        assert_eq!(pools.len(), 2);
        let pool = &pools[0];
        assert_eq!(pool.id, 53114);
        assert_eq!(pool.name, "MEAT by nsfuwu");
        assert_eq!(pool.category, PoolCategory::Series);
        assert_eq!(pool.creator.as_deref(), Some("VaurukBoy"));
        assert_eq!(pool.post_ids.len(), 51);
        assert_eq!(pool.post_ids[..2], [6062592, 6062593]);
    }

    #[test]
    fn decodes_image_posts() {
        let posts = posts(include_str!("../../tests/fixtures/posts_image.json"));
        assert_eq!(posts.len(), 3);
        let post = &posts[0];
        assert_eq!(post.id, 6783874);
        assert_eq!(post.file.ext, "png");
        assert!(
            post.file
                .url
                .as_deref()
                .unwrap()
                .starts_with("https://static1.e621.net/data/")
        );
        assert!(post.preview.url.is_some());
        assert!(post.sample.as_ref().unwrap().url.is_some());
        assert_eq!(post.video, None);
        assert!(!post.deleted);
    }

    #[test]
    fn decodes_gif_posts() {
        let posts = posts(include_str!("../../tests/fixtures/posts_gif.json"));
        assert!(
            posts
                .iter()
                .all(|post| post.file.ext == "gif" && post.video.is_none())
        );
    }

    #[test]
    fn collects_the_video_encodings_of_webm_posts() {
        let posts = posts(include_str!("../../tests/fixtures/posts_webm.json"));
        let post = &posts[0];
        assert_eq!(post.id, 6783863);
        assert_eq!(post.rating, Rating::Safe);
        let video = post.video.as_ref().unwrap();
        let p720 = video.p720.as_ref().unwrap();
        assert_eq!(
            p720.url,
            "https://static1.e621.net/data/sample/d9/ca/d9ca582808cf057725795ae3bc6b58be_720p.mp4"
        );
        assert_eq!((p720.width, p720.height), (810, 1440));
        assert_eq!(p720.codec.as_deref(), Some("avc1.4D401E"));
        assert!(video.p480.is_some());
        assert!(video.mp4.as_ref().unwrap().url.ends_with("_alt.mp4"));
        assert!(video.original.as_ref().unwrap().url.ends_with(".webm"));
        assert_eq!(post.tags.artist, ["natural_habitok"]);
        assert_eq!(post.uploader.name.as_deref(), Some("translator8skunk"));
    }

    #[test]
    fn keeps_metadata_of_deleted_posts_without_urls() {
        let posts = posts(include_str!("../../tests/fixtures/posts_deleted.json"));
        let post = &posts[0];
        assert!(post.deleted);
        assert_eq!(post.file.url, None);
        assert_eq!(post.preview.url, None);
        assert_eq!(post.sample.as_ref().unwrap().url, None);
        assert_eq!(post.file.md5, "93c062477ee6b163a7c4eee1a39bd13d");
        assert_eq!(post.score.down, -2);
        assert_eq!(post.tags.character, ["fluttershy_(mlp)"]);
    }

    #[test]
    fn treats_a_null_user_id_as_anonymous() {
        let raw: RawUser =
            serde_json::from_str(include_str!("../../tests/fixtures/users_me_anonymous.json"))
                .unwrap();
        assert_eq!(raw.into_user(), None);
    }

    #[test]
    fn decodes_autocomplete_suggestions() {
        let suggestions: Vec<TagSuggestion> =
            serde_json::from_str(include_str!("../../tests/fixtures/autocomplete.json")).unwrap();
        assert_eq!(suggestions.len(), 10);
        assert_eq!(suggestions[0].name, "canine");
        assert_eq!(suggestions[0].category, TagCategory::Species);
        assert_eq!(suggestions[0].antecedent_name.as_deref(), Some("wolfgirl"));
        assert_eq!(suggestions[4].category, TagCategory::Character);
        assert_eq!(suggestions[8].category, TagCategory::General);
    }

    #[test]
    fn rejects_unknown_tag_categories() {
        let result: Result<TagSuggestion, _> = serde_json::from_str(
            r#"{"id":1,"name":"x","post_count":1,"category":42,"antecedent_name":null}"#,
        );
        assert!(result.is_err());
    }
}
