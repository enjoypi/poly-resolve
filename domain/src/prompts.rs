use core::fmt::Debug;

use crate::AppError;

pub trait Library: Debug + Sync {
    fn list(&self) -> Result<Vec<Prompt>, AppError>;
    fn save(&self, prompt: &Prompt) -> Result<Option<()>, AppError>;
}

#[derive(Clone, Debug)]
pub struct Prompt {
    name: String,
    text: String,
}

impl Prompt {
    #[must_use = "名字需用于定位文件"]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use = "提示词需交给存储"]
    pub const fn new(name: String, text: String) -> Self {
        Self { name, text }
    }

    #[must_use = "正文需写入文件"]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug)]
pub(crate) struct Vacant;

impl Library for Vacant {
    fn list(&self) -> Result<Vec<Prompt>, AppError> {
        Ok(Vec::new())
    }

    fn save(&self, _prompt: &Prompt) -> Result<Option<()>, AppError> {
        Ok(None)
    }
}
