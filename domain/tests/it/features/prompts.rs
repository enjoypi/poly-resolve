use std::sync::Mutex;

use domain::AppError;
use domain::prompts::{Library, Prompt};
use serde_json::{Error, Value, from_str, json};

use crate::support::{Fixture, local};

const SESSION: &str = r#"{"text":"ctx:{messages}"}"#;

type Reply = Result<Value, u16>;

#[derive(Debug, Default)]
struct Shelf {
    failing: Option<()>,
    saved: Mutex<String>,
}

impl Library for Shelf {
    fn list(&self) -> Result<Vec<Prompt>, AppError> {
        self.failing.map_or_else(
            || {
                Ok(vec![
                    Prompt::new("session".to_owned(), "ctx:{messages}".to_owned()),
                    Prompt::new("screen".to_owned(), "judge".to_owned()),
                    Prompt::new("extra".to_owned(), "x".to_owned()),
                ])
            },
            |()| Err(AppError::Storage),
        )
    }

    fn save(&self, prompt: &Prompt) -> Result<Option<()>, AppError> {
        if self.failing.is_some() {
            return Err(AppError::Storage);
        }
        prompt
            .text()
            .clone_into(&mut *self.saved.lock().map_err(|_err| AppError::Storage)?);
        Ok((prompt.name() != "screen").then_some(()))
    }
}

fn broken() -> Shelf {
    Shelf {
        failing: Some(()),
        ..Shelf::default()
    }
}

fn check(name: &str, text: &str) -> Result<Reply, Error> {
    let body = json!({"name": name, "text": text}).to_string();
    reply(Fixture::new()?.dispatch(&local("prompts", "check", &body)))
}

#[test]
fn check_accepts_each_placeholder_exactly_once() {
    for (name, text) in [
        ("session", "a {messages}"),
        ("followup", "{messages} b"),
        ("topic", "which {segment}"),
        ("urgent", "urgent {line}"),
        ("screen", "judge"),
    ] {
        assert_eq!(
            check(name, text).unwrap(),
            Ok(json!({"name": name})),
            "{name}"
        );
    }
}

#[test]
fn check_refuses_blank_missing_or_repeated_placeholders() {
    assert_eq!(check("session", "  \n").unwrap(), Err(422));
    assert_eq!(check("screen", "").unwrap(), Err(422));
    assert_eq!(check("urgent", "no line").unwrap(), Err(422));
    assert_eq!(check("followup", "{messages}{messages}").unwrap(), Err(422));
    assert_eq!(check("nope", "x").unwrap(), Err(404));
}

#[test]
fn check_rejects_malformed_request() {
    let reply = reply(
        Fixture::new()
            .unwrap()
            .dispatch(&local("prompts", "check", "nope")),
    );
    assert_eq!(reply.unwrap(), Err(400));
}

fn list(library: &dyn Library) -> Result<Reply, Error> {
    reply(Fixture::new()?.dispatch_prompts(library, &local("prompts", "list", "")))
}

#[test]
fn list_reports_storage_failure() {
    assert_eq!(list(&broken()).unwrap(), Err(503));
}

#[test]
fn list_shows_text_and_required_placeholders() {
    let reply = list(&Shelf::default()).unwrap().unwrap();
    assert_eq!(
        reply,
        json!({"items": [
            {"name": "session", "required": ["{messages}"], "text": "ctx:{messages}"},
            {"name": "screen", "required": [], "text": "judge"},
            {"name": "extra", "required": [], "text": "x"},
        ]})
    );
}

#[test]
fn list_without_library_is_empty() {
    let reply = reply(
        Fixture::new()
            .unwrap()
            .dispatch(&local("prompts", "list", "")),
    );
    assert_eq!(reply.unwrap(), Ok(json!({"items": []})));
}

fn reply(result: Result<String, AppError>) -> Result<Reply, Error> {
    match result {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

fn update(library: &dyn Library, query: &str, body: &str) -> Result<Reply, Error> {
    let call = local("prompts", "update", body).with_params(query);
    reply(Fixture::new()?.dispatch_prompts(library, &call))
}

#[test]
fn update_refuses_invalid_text_before_saving() {
    let shelf = Shelf::default();
    let body = r#"{"text":"lost"}"#;
    assert_eq!(update(&shelf, "name=session", body).unwrap(), Err(422));
    assert_eq!(update(&shelf, "name=nope", SESSION).unwrap(), Err(404));
    assert_eq!(*shelf.saved.lock().unwrap(), "");
}

#[test]
fn update_rejects_malformed_requests() {
    let shelf = Shelf::default();
    assert_eq!(update(&shelf, "", SESSION).unwrap(), Err(400));
    assert_eq!(update(&shelf, "name=session", "nope").unwrap(), Err(400));
}

#[test]
fn update_reports_unconfigured_and_storage_failures() {
    let screen = r#"{"text":"judge"}"#;
    assert_eq!(
        update(&Shelf::default(), "name=screen", screen).unwrap(),
        Err(404)
    );
    assert_eq!(
        update(&broken(), "name=session", SESSION).unwrap(),
        Err(503)
    );
    let call = local("prompts", "update", SESSION).with_params("name=session");
    let vacant = reply(Fixture::new().unwrap().dispatch(&call));
    assert_eq!(vacant.unwrap(), Err(404));
}

#[test]
fn update_saves_the_text() {
    let shelf = Shelf::default();
    let done = update(&shelf, "name=session", SESSION).unwrap();
    assert_eq!(done, Ok(json!({"name": "session"})));
    assert_eq!(*shelf.saved.lock().unwrap(), "ctx:{messages}");
}
