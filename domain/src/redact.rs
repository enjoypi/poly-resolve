use core::cmp::Reverse;

use crate::AppError;
use crate::features::triage::redact::{Mask, Rule as Step};
use crate::review::Rule;

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
            .map(|rule| Step::new(rule.pattern(), rule.placeholder()))
            .collect();
        Ok(Self {
            mask: Mask::new(&steps)?,
        })
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

#[must_use = "占位符需交给审核页"]
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
