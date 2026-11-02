use std::fs;
use std::io;
use std::path::PathBuf;

use domain::AppError;
use domain::prompts::{Library, Prompt};

use super::config::Screen;

const NAMES: [&str; 4] = ["followup", "session", "topic", "urgent"];
const SCREEN: &str = "screen";
const SUFFIX: &str = "md";
const TEMPORARY: &str = "md.tmp";

#[derive(Debug)]
pub struct Folder {
    dir: PathBuf,
    names: Vec<&'static str>,
}

impl Folder {
    pub(crate) fn names(&self) -> &[&'static str] {
        &self.names
    }

    pub(crate) fn new(dir: PathBuf, screen: Option<&Screen>) -> Self {
        let names = NAMES.into_iter().chain(screen.map(|_| SCREEN)).collect();
        Self { dir, names }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name).with_extension(SUFFIX)
    }

    pub(crate) fn read(&self, name: &str) -> Option<String> {
        fs::read_to_string(self.path(name))
            .map_err(|err| tracing::warn!(name, error = %err.kind(), "prompt unreadable"))
            .ok()
    }
}

impl Library for Folder {
    fn list(&self) -> Result<Vec<Prompt>, AppError> {
        self.names
            .iter()
            .map(|name| {
                fs::read_to_string(self.path(name))
                    .map(|text| Prompt::new((*name).to_owned(), text))
                    .map_err(|err| failed(name, &err))
            })
            .collect()
    }

    fn save(&self, prompt: &Prompt) -> Result<Option<()>, AppError> {
        let Some(name) = self.names.iter().find(|name| **name == prompt.name()) else {
            return Ok(None);
        };
        let path = self.path(name);
        let staged = path.with_extension(TEMPORARY);
        fs::write(&staged, prompt.text()).map_err(|err| failed(name, &err))?;
        fs::rename(&staged, &path).map_err(|err| {
            drop(fs::remove_file(&staged));
            failed(name, &err)
        })?;
        tracing::info!(name, len = prompt.text().len(), "prompt saved");
        Ok(Some(()))
    }
}

fn failed(name: &str, err: &io::Error) -> AppError {
    tracing::warn!(name, error = %err.kind(), "prompt storage failed");
    AppError::Storage
}
