use core::cmp::Reverse;

use blake3::{Hasher, derive_key};

use crate::AppError;
use crate::features::triage::redact::{HASH, Mask, Rule as Step};

const DIGEST_CHARS: usize = 32;
const DOMAIN: &str = "poly-resolve triage placeholder";
const PLAIN: &str = "-";

#[derive(Debug)]
pub struct Redactor {
    mask: Mask,
}

impl Redactor {
    #[must_use = "脱敏结果需写出"]
    pub fn apply(&self, text: &str) -> String {
        self.mask.apply(text)
    }

    pub fn new(rules: &[Rule]) -> Result<Self, AppError> {
        let steps: Vec<Step> = rules
            .iter()
            .map(|rule| Step::new(&rule.pattern, &rule.placeholder.replace(HASH, PLAIN)))
            .collect();
        Ok(Self {
            mask: Mask::new(&steps, None)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Rule {
    pattern: String,
    placeholder: String,
}

impl Rule {
    #[must_use = "规则需交给日志脱敏"]
    pub const fn new(pattern: String, placeholder: String) -> Self {
        Self {
            pattern,
            placeholder,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Seal {
    chars: usize,
    hasher: Hasher,
}

impl Seal {
    pub(crate) const fn chars(&self) -> usize {
        self.chars
    }

    pub(crate) fn digest(&self, text: &str) -> String {
        let normal = text.split_whitespace().collect::<Vec<_>>().join(" ");
        self.hex(&normal, DIGEST_CHARS)
    }

    fn hex(&self, value: &str, chars: usize) -> String {
        self.hasher
            .clone()
            .update(value.trim().to_lowercase().as_bytes())
            .finalize()
            .to_hex()
            .chars()
            .take(chars)
            .collect()
    }

    #[must_use = "密钥需交给 Context"]
    pub fn new(material: &str, chars: u32) -> Self {
        Self {
            chars: usize::try_from(chars).unwrap_or(usize::MAX),
            hasher: Hasher::new_keyed(&derive_key(DOMAIN, material.as_bytes())),
        }
    }

    pub(crate) fn tag(&self, value: &str) -> String {
        self.hex(value, self.chars)
    }
}

fn literal(template: &str) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some((head, tail)) = rest.split_once('$') {
        out.push_str(head);
        let mut letters = tail.chars();
        rest = match letters.next() {
            Some('$') => {
                out.push('$');
                letters.as_str()
            }
            Some('{') => letters
                .as_str()
                .split_once('}')
                .map_or("", |(_name, more)| more),
            _ => tail
                .trim_start_matches(|letter: char| letter.is_ascii_alphanumeric() || letter == '_'),
        };
    }
    out.push_str(rest);
    out
}

#[must_use = "占位符需交给外发记录页"]
pub fn marks(placeholders: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = placeholders
        .iter()
        .map(|template| literal(template))
        .filter(|text| !text.is_empty())
        .collect();
    found.sort_by_key(|text| (Reverse(text.chars().count()), text.clone()));
    found.dedup();
    found
}
