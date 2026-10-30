mod line;
mod pack;
mod prompt;
mod redact;
mod reply;
mod screen;
mod screened;
mod sort;

use serde_json::{Error, Value, from_str};

use crate::support::{Fixture, local};

type Reply = Result<Value, u16>;

fn call(action: &str, request: &str) -> Result<Reply, Error> {
    match Fixture::new()?.dispatch(&local("triage", action, request)) {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

fn patched(mut request: Value, key: &str, value: &Value) -> Value {
    if let Some(map) = request.as_object_mut() {
        drop(map.insert(key.to_owned(), value.clone()));
    }
    request
}
