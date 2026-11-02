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
