//! The account blacklist, evaluated the way e621's own site does it in the browser.
//!
//! Ported from e621ng `app/javascript/src/js/core/blacklist/` (`Filter.ts`, `FilterToken.ts`,
//! `FilterUtilities.ts`) and the post fields of `models/PostCache.ts`, quirks included, so a
//! blacklist hides here exactly what it hides on the site (docs/research.md §2).

use std::collections::HashSet;

use crate::api::models::{Post, Rating};

/// Every line of the blacklist that is not blank or a comment.
#[derive(Debug, Default)]
pub struct Blacklist {
    filters: Vec<Filter>,
}

/// One line: every plain token must match, and at least one `~` token if there are any.
#[derive(Debug)]
struct Filter {
    required: Vec<Token>,
    optional: Vec<Token>,
}

#[derive(Debug)]
struct Token {
    inverted: bool,
    test: Test,
}

#[derive(Debug)]
enum Test {
    Tag(String),
    Wildcard(String),
    Number(NumberField, Comparison),
    Status(String),
    Rating(Option<Rating>),
    Type(String),
    Favorited,
    User(String),
    Username(String),
    Pool(u64),
    /// A value the site cannot use either (a range for a text metatag, say); never matches.
    Never,
}

#[derive(Clone, Copy, Debug)]
enum NumberField {
    TagCount,
    Id,
    Width,
    Height,
    FileSize,
    Score,
    FavCount,
    UserId,
}

/// `None` stands for the site's `NaN`: a value that compares false with everything.
#[derive(Debug)]
enum Comparison {
    Equal(Option<i64>),
    Less(Option<i64>),
    LessOrEqual(Option<i64>),
    Greater(Option<i64>),
    GreaterOrEqual(Option<i64>),
    Between(Option<i64>, Option<i64>),
}

const METATAGS: &[&str] = &[
    "tag", "tagcount", "id", "status", "rating", "type", "width", "height", "filesize", "score",
    "favcount", "fav", "uploader", "user", "userid", "username", "pool",
];

impl Blacklist {
    /// The blacklist as e621 stores it: one filter per line.
    pub fn parse(text: &str) -> Self {
        Self {
            filters: text.lines().filter_map(Filter::parse).collect(),
        }
    }

    pub fn matches(&self, post: &Post) -> bool {
        let tags = all_tags(post);
        self.filters
            .iter()
            .any(|filter| filter.matches(post, &tags))
    }
}

impl Filter {
    fn parse(line: &str) -> Option<Self> {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let line = line.to_lowercase();
        let line = line
            .find(" #")
            .map_or(line.as_str(), |comment| &line[..comment]);
        let mut seen = HashSet::new();
        let (mut required, mut optional) = (Vec::new(), Vec::new());
        for word in line.split(' ').filter(|word| !word.trim().is_empty()) {
            if !seen.insert(word) {
                continue;
            }
            let (token, is_optional) = Token::parse(word);
            if is_optional {
                optional.push(token);
            } else {
                required.push(token);
            }
        }
        Some(Self { required, optional })
    }

    fn matches(&self, post: &Post, tags: &[&str]) -> bool {
        self.required.iter().all(|token| token.matches(post, tags))
            && (self.optional.is_empty()
                || self.optional.iter().any(|token| token.matches(post, tags)))
    }
}

impl Token {
    /// Also says whether the token is optional (`~`).
    fn parse(word: &str) -> (Self, bool) {
        let raw = word.trim().to_lowercase();
        let rest = raw.trim_start_matches(['~', '-']);
        let prefix = &raw[..raw.len() - rest.len()];
        let token = Self {
            inverted: prefix.contains('-'),
            test: Test::parse(rest),
        };
        (token, prefix.contains('~'))
    }

    fn matches(&self, post: &Post, tags: &[&str]) -> bool {
        self.test.matches(post, tags) != self.inverted
    }
}

impl Test {
    fn parse(raw: &str) -> Self {
        let kind = match raw.split_once(':') {
            Some((name, _)) if METATAGS.contains(&name) => name,
            _ => "tag",
        };
        if kind == "tag" {
            return if raw.contains('*') {
                Self::Wildcard(raw.to_owned())
            } else {
                Self::Tag(raw.to_owned())
            };
        }
        let value = &raw[kind.len() + 1..];
        let number = |field| Self::Number(field, Comparison::parse(value, field));
        match kind {
            "tagcount" => number(NumberField::TagCount),
            "id" => number(NumberField::Id),
            "width" => number(NumberField::Width),
            "height" => number(NumberField::Height),
            "filesize" => number(NumberField::FileSize),
            "score" => number(NumberField::Score),
            "favcount" => number(NumberField::FavCount),
            "userid" => number(NumberField::UserId),
            "fav" => Self::Favorited,
            _ => match Comparison::text_value(value) {
                None => Self::Never,
                Some(value) => match kind {
                    "status" => Self::Status(value.to_owned()),
                    "rating" => Self::Rating(parse_rating(value)),
                    "type" => Self::Type(value.to_owned()),
                    "uploader" | "user" => Self::User(value.to_owned()),
                    "username" => Self::Username(value.to_owned()),
                    "pool" => Self::Pool(
                        parse_int(value)
                            .and_then(|id| u64::try_from(id).ok())
                            .unwrap_or(0),
                    ),
                    _ => unreachable!("every metatag is handled"),
                },
            },
        }
    }

    fn matches(&self, post: &Post, tags: &[&str]) -> bool {
        match self {
            Self::Tag(tag) => tags.contains(&tag.as_str()),
            Self::Wildcard(pattern) => tags.iter().any(|tag| glob(pattern, tag)),
            Self::Number(field, comparison) => comparison.test(field.of(post, tags)),
            Self::Status(flag) => match flag.as_str() {
                "pending" => post.pending,
                "flagged" => post.flagged,
                "deleted" => post.deleted,
                _ => false,
            },
            Self::Rating(rating) => Some(post.rating) == *rating,
            Self::Type(ext) => post.file.ext == *ext,
            Self::Favorited => post.is_favorited,
            Self::User(user) => match user.strip_prefix('!') {
                Some(id) => parse_int(id) == i64::try_from(post.uploader.id).ok(),
                None => uploader(post) == *user,
            },
            Self::Username(name) => uploader(post) == *name,
            Self::Pool(id) => post.pools.contains(id),
            Self::Never => false,
        }
    }
}

impl NumberField {
    fn of(self, post: &Post, tags: &[&str]) -> i64 {
        let number = |value: u64| i64::try_from(value).unwrap_or(i64::MAX);
        match self {
            Self::TagCount => number(tags.len() as u64),
            Self::Id => number(post.id),
            Self::Width => post.file.width.into(),
            Self::Height => post.file.height.into(),
            Self::FileSize => number(post.file.size),
            Self::Score => post.score.total,
            Self::FavCount => number(post.fav_count),
            Self::UserId => number(post.uploader.id),
        }
    }
}

impl Comparison {
    fn parse(raw: &str, field: NumberField) -> Self {
        let value = |text: &str| match field {
            NumberField::FileSize => parse_file_size(text),
            _ => parse_int(text),
        };
        if raw.contains("..") {
            if let Some(high) = raw.strip_prefix("..") {
                return Self::LessOrEqual(value(high));
            }
            if let Some(low) = raw.strip_suffix("..") {
                return Self::GreaterOrEqual(value(low));
            }
            let parts: Vec<&str> = raw.split("..").collect();
            return match parts[..] {
                [low, high] => Self::Between(value(low), value(high)),
                _ => Self::Equal(None),
            };
        }
        let (operator, rest) = split_operator(raw);
        let number = value(rest);
        match operator {
            "<" => Self::Less(number),
            "<=" => Self::LessOrEqual(number),
            ">" => Self::Greater(number),
            ">=" => Self::GreaterOrEqual(number),
            // The site's filesize metatag is fuzzy: an exact size means within 5%.
            _ if matches!(field, NumberField::FileSize) => Self::Between(
                number.map(|size| (size as f64 * 0.95) as i64),
                number.map(|size| (size as f64 * 1.05) as i64),
            ),
            _ => Self::Equal(number),
        }
    }

    /// The value of a text metatag after the site strips its comparison. An open range keeps
    /// its one end as the value; a closed or broken one leaves nothing usable.
    fn text_value(raw: &str) -> Option<&str> {
        if raw.contains("..") {
            return raw.strip_prefix("..").or_else(|| raw.strip_suffix(".."));
        }
        Some(split_operator(raw).1)
    }

    fn test(&self, value: i64) -> bool {
        match *self {
            Self::Equal(Some(target)) => value == target,
            Self::Less(Some(target)) => value < target,
            Self::LessOrEqual(Some(target)) => value <= target,
            Self::Greater(Some(target)) => value > target,
            Self::GreaterOrEqual(Some(target)) => value >= target,
            Self::Between(Some(low), Some(high)) => value >= low && value <= high,
            _ => false,
        }
    }
}

/// The comparison at the start of a value, as the site reads it: one or two of `<>=`, with
/// `=<` and `=>` turned around. Anything else, a lone `=` included, is equality and stays in
/// the value.
fn split_operator(raw: &str) -> (&str, &str) {
    let length = raw
        .bytes()
        .take(2)
        .take_while(|byte| b"<>=".contains(byte))
        .count();
    let operator = match &raw[..length] {
        "<=" | "=<" => "<=",
        ">=" | "=>" => ">=",
        "<" => "<",
        ">" => ">",
        _ => return ("=", raw),
    };
    (operator, &raw[length..])
}

/// JavaScript's `parseInt`: an optional sign and the leading digits; `None` for `NaN`.
fn parse_int(text: &str) -> Option<i64> {
    let text = text.trim_start();
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let end = digits.bytes().take_while(u8::is_ascii_digit).count();
    let value: i64 = digits[..end].parse().ok()?;
    Some(if negative { -value } else { value })
}

/// `5mb`, `300kb`, `1024b` or plain bytes, with decimals allowed.
fn parse_file_size(text: &str) -> Option<i64> {
    let digits = text
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
        .count();
    let (number, unit) = text.split_at(digits);
    if number.starts_with('.') || number.ends_with('.') || number.matches('.').count() > 1 {
        return None;
    }
    let number: f64 = number.parse().ok()?;
    let scale = match unit.trim_start() {
        "" | "b" => 1.0,
        "kb" => 1024.0,
        "mb" => 1_048_576.0,
        _ => return None,
    };
    Some((number * scale).floor() as i64)
}

fn parse_rating(value: &str) -> Option<Rating> {
    match value {
        "safe" | "s" => Some(Rating::Safe),
        "questionable" | "q" => Some(Rating::Questionable),
        "explicit" | "e" => Some(Rating::Explicit),
        _ => None,
    }
}

/// `*` matches any run of characters, and nothing else is special.
fn glob(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

fn all_tags(post: &Post) -> Vec<&str> {
    let tags = &post.tags;
    [
        &tags.general,
        &tags.artist,
        &tags.contributor,
        &tags.copyright,
        &tags.character,
        &tags.species,
        &tags.invalid,
        &tags.meta,
        &tags.lore,
    ]
    .into_iter()
    .flatten()
    .map(String::as_str)
    .collect()
}

fn uploader(post: &Post) -> String {
    post.uploader
        .name
        .as_deref()
        .unwrap_or_default()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::decode_posts;

    /// The WebM fixture post: rating s, 1080x1920 webm of 6662762 bytes, score 2, 1 favorite,
    /// pending, uploaded by translator8skunk (2262283), tags such as `rabbit`, `frog`, `yoga`
    /// and `natural_habitok`.
    fn post() -> Post {
        decode_posts(include_str!("../tests/fixtures/posts_webm.json")).remove(0)
    }

    fn hides(blacklist: &str) -> bool {
        Blacklist::parse(blacklist).matches(&post())
    }

    #[test]
    fn needs_every_plain_tag_of_a_line() {
        assert!(hides("rabbit frog"));
        assert!(!hides("rabbit wolf"));
        assert!(hides("wolf\nrabbit"));
    }

    #[test]
    fn needs_one_of_the_optional_tags() {
        assert!(hides("~wolf ~rabbit"));
        assert!(!hides("~wolf ~dragon"));
        assert!(hides("frog ~wolf ~rabbit"));
        assert!(!hides("wolf ~rabbit"));
    }

    #[test]
    fn inverts_minus_tokens() {
        assert!(!hides("rabbit -frog"));
        assert!(hides("rabbit -wolf"));
        assert!(hides("-wolf"));
        assert!(hides("~-wolf ~dragon"));
    }

    #[test]
    fn ignores_comments_case_and_repeated_tokens() {
        assert!(!hides("# rabbit"));
        assert!(hides("RABBIT # why"));
        assert!(!hides("wolf # rabbit"));
        assert!(hides("rabbit rabbit"));
        assert!(!hides("\n  \n"));
    }

    #[test]
    fn matches_wildcards() {
        assert!(hides("rab*"));
        assert!(hides("*_habitok"));
        assert!(hides("natural*hab*"));
        assert!(!hides("wol*"));
        assert!(glob("a*", "a"));
        assert!(!glob("ab*b", "ab"));
    }

    #[test]
    fn compares_numbers_and_ranges() {
        assert!(hides("score:2"));
        assert!(hides("score:<3"));
        assert!(hides("score:=<2"));
        assert!(!hides("score:>2"));
        assert!(hides("score:1..5"));
        assert!(hides("score:..2"));
        assert!(!hides("score:3.."));
        assert!(hides("width:1080 height:>=1920"));
        assert!(hides("favcount:1"));
        assert!(hides("id:6783863"));
        assert!(hides("userid:2262283"));
        assert!(hides("tagcount:>50"));
    }

    #[test]
    fn treats_unreadable_numbers_like_the_site() {
        // `=` is kept in the value, which then is not a number.
        assert!(!hides("score:=2"));
        assert!(!hides("score:abc"));
        assert!(!hides("score:1..2..3"));
        assert!(hides("score:2abc"));
    }

    #[test]
    fn reads_file_sizes_with_five_percent_slack() {
        assert!(hides("filesize:6.5mb"));
        assert!(hides("filesize:6662762"));
        assert!(!hides("filesize:5mb"));
        assert!(hides("filesize:>5mb"));
        assert!(hides("filesize:6000kb..7mb"));
    }

    #[test]
    fn matches_text_metatags() {
        assert!(hides("rating:s"));
        assert!(hides("rating:safe"));
        assert!(!hides("rating:e"));
        assert!(!hides("rating:nonsense"));
        assert!(hides("type:webm"));
        assert!(hides("status:pending"));
        assert!(!hides("status:deleted"));
        assert!(hides("user:translator8skunk"));
        assert!(hides("uploader:Translator8Skunk"));
        assert!(hides("user:!2262283"));
        assert!(hides("username:translator8skunk"));
        assert!(!hides("user:a..b"));
        assert!(hides("user:..translator8skunk"));
        assert!(!hides("pool:12"));
        assert!(!hides("fav:me"));
    }

    #[test]
    fn treats_unknown_metatags_as_tags() {
        assert!(!hides("source:youtube"));
        assert!(Blacklist::parse("tag:rabbit").filters.len() == 1);
    }

    #[test]
    fn applies_the_anonymous_default_blacklist() {
        let me: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/users_me_anonymous.json"))
                .unwrap();
        let blacklist = Blacklist::parse(me["blacklisted_tags"].as_str().unwrap());
        assert!(!blacklist.filters.is_empty());
        let mut post = post();
        assert!(!blacklist.matches(&post));
        post.tags.general.push("gore".into());
        assert!(blacklist.matches(&post));
        post.tags.general.pop();
        post.tags.general.push("young".into());
        assert!(
            !blacklist.matches(&post),
            "young -rating:s spares safe posts"
        );
        post.rating = Rating::Explicit;
        assert!(blacklist.matches(&post));
    }
}
