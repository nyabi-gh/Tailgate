//! The file name rule for downloads: a template such as `{id}_{artist}`; the extension is
//! added after it (docs/report.md §3).

use serde::Serialize;

pub const DEFAULT_TEMPLATE: &str = "{id}_{artist}";
/// Removed next to a token that has no value, so `{id}_{artist}` without an artist is `42`.
const SEPARATORS: &[char] = &['_', '-', '.', ' '];
/// Long tag names stay readable without crowding out the rest of the name.
const MAX_TAG_CHARS: usize = 60;
/// Leaves room for the extension and `.part` within the 255 bytes most file systems allow.
const MAX_STEM_BYTES: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Token {
    Id,
    Md5,
    Artist,
    Copyright,
    Character,
    Rating,
}

impl Token {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "id" => Self::Id,
            "md5" => Self::Md5,
            "artist" => Self::Artist,
            "copyright" => Self::Copyright,
            "character" => Self::Character,
            "rating" => Self::Rating,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Part {
    Text(String),
    Token(Token),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NameTemplate(Vec<Part>);

#[derive(Clone, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum TemplateError {
    #[error("there is no {{{token}}}")]
    UnknownToken { token: String },
    #[error("a `{{` is not closed")]
    Unclosed,
    /// Without either, two posts could get the same name and the second would pass for saved.
    #[error("the name needs {{id}} or {{md5}}")]
    NotUnique,
    #[error("a file name cannot hold a folder separator")]
    Separator,
}

/// The post the settings screen shows a rule's result with.
pub const EXAMPLE: NameFields<'static> = NameFields {
    id: 4512873,
    md5: "9d3a5d7e8b6c4f21a0e1b2c3d4e5f607",
    artist: Some("artist_name"),
    copyright: Some("original"),
    character: Some("character_name"),
    rating: "s",
};

/// What a name can draw on. Tags are the first of their kind that the post has.
pub struct NameFields<'a> {
    pub id: u64,
    pub md5: &'a str,
    pub artist: Option<&'a str>,
    pub copyright: Option<&'a str>,
    pub character: Option<&'a str>,
    pub rating: &'a str,
}

impl NameTemplate {
    pub fn parse(text: &str) -> Result<Self, TemplateError> {
        if text.contains(['/', '\\']) {
            return Err(TemplateError::Separator);
        }
        let mut parts = Vec::new();
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            if open > 0 {
                parts.push(Part::Text(rest[..open].to_owned()));
            }
            let after = &rest[open + 1..];
            let close = after.find('}').ok_or(TemplateError::Unclosed)?;
            let name = &after[..close];
            let token = Token::parse(name).ok_or_else(|| TemplateError::UnknownToken {
                token: name.to_owned(),
            })?;
            parts.push(Part::Token(token));
            rest = &after[close + 1..];
        }
        if !rest.is_empty() {
            parts.push(Part::Text(rest.to_owned()));
        }
        if !parts
            .iter()
            .any(|part| matches!(part, Part::Token(Token::Id | Token::Md5)))
        {
            return Err(TemplateError::NotUnique);
        }
        Ok(Self(parts))
    }

    /// The file name without its extension, with characters a file name may not hold on
    /// Windows or macOS replaced by `_`.
    pub fn stem(&self, fields: &NameFields) -> String {
        let mut stem = String::new();
        // A token without a value at the very start takes the separator after it instead.
        let mut drop_leading_separator = false;
        for part in &self.0 {
            match part {
                Part::Text(text) => {
                    let text = if drop_leading_separator {
                        text.trim_start_matches(SEPARATORS)
                    } else {
                        text
                    };
                    stem.push_str(text);
                    drop_leading_separator = false;
                }
                Part::Token(token) => match value(*token, fields) {
                    Some(value) => {
                        stem.push_str(&value);
                        drop_leading_separator = false;
                    }
                    None => {
                        let kept = stem.trim_end_matches(SEPARATORS).len();
                        drop_leading_separator = kept == stem.len();
                        stem.truncate(kept);
                    }
                },
            }
        }
        let mut stem = clean(&stem);
        if stem.len() > MAX_STEM_BYTES {
            let mut end = MAX_STEM_BYTES;
            while !stem.is_char_boundary(end) {
                end -= 1;
            }
            stem.truncate(end);
        }
        // Windows drops trailing dots and spaces from a name, which would no longer match.
        stem.trim_end_matches(['.', ' ']).to_owned()
    }
}

impl Default for NameTemplate {
    fn default() -> Self {
        Self::parse(DEFAULT_TEMPLATE).expect("the default template is valid")
    }
}

fn value(token: Token, fields: &NameFields) -> Option<String> {
    let tag = |tag: Option<&str>| {
        tag.filter(|tag| !tag.is_empty())
            .map(|tag| tag.chars().take(MAX_TAG_CHARS).collect())
    };
    match token {
        Token::Id => Some(fields.id.to_string()),
        Token::Md5 => Some(fields.md5.to_owned()),
        Token::Artist => tag(fields.artist),
        Token::Copyright => tag(fields.copyright),
        Token::Character => tag(fields.character),
        Token::Rating => Some(fields.rating.to_owned()),
    }
}

pub fn clean(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() || "-_.() ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> NameFields<'static> {
        NameFields {
            id: 42,
            md5: "0123456789abcdef0123456789abcdef",
            artist: Some("foo_bar"),
            copyright: Some("pokemon"),
            character: None,
            rating: "s",
        }
    }

    fn stem(template: &str) -> String {
        NameTemplate::parse(template).unwrap().stem(&fields())
    }

    #[test]
    fn fills_in_the_tokens() {
        assert_eq!(stem("{id}_{artist}"), "42_foo_bar");
        assert_eq!(
            stem("{copyright} - {artist} - {md5}"),
            "pokemon - foo_bar - 0123456789abcdef0123456789abcdef"
        );
        assert_eq!(stem("e621 {rating} {id}"), "e621 s 42");
    }

    #[test]
    fn leaves_out_a_token_without_a_value_and_its_separator() {
        assert_eq!(stem("{id}_{character}"), "42");
        assert_eq!(stem("{character}_{id}"), "42");
        assert_eq!(stem("{id} - {character} - {artist}"), "42 - foo_bar");
    }

    #[test]
    fn replaces_characters_a_file_name_cannot_hold() {
        let fields = NameFields {
            artist: Some("a:b?c*"),
            ..fields()
        };
        let template = NameTemplate::parse("{id}_{artist}").unwrap();
        assert_eq!(template.stem(&fields), "42_a_b_c_");
    }

    #[test]
    fn keeps_names_within_the_length_limit() {
        let long = "x".repeat(300);
        let fields = NameFields {
            artist: Some(&long),
            copyright: Some(&long),
            character: Some(&long),
            ..fields()
        };
        let template =
            NameTemplate::parse("{id} {artist} {copyright} {character} {artist}").unwrap();
        let stem = template.stem(&fields);
        assert!(stem.len() <= MAX_STEM_BYTES);
        assert!(stem.starts_with("42 "));
        assert!(!stem.ends_with(' '));
    }

    #[test]
    fn refuses_templates_that_cannot_name_a_file() {
        assert_eq!(
            NameTemplate::parse("{artist}"),
            Err(TemplateError::NotUnique)
        );
        assert_eq!(NameTemplate::parse(""), Err(TemplateError::NotUnique));
        assert_eq!(
            NameTemplate::parse("{id}_{artsit}"),
            Err(TemplateError::UnknownToken {
                token: "artsit".into()
            })
        );
        assert_eq!(
            NameTemplate::parse("{id}_{artist"),
            Err(TemplateError::Unclosed)
        );
        assert_eq!(
            NameTemplate::parse("{artist}/{id}"),
            Err(TemplateError::Separator)
        );
    }
}
