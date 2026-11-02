use std::io;
use std::process::Output;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Caller<'caller> {
    feature: &'caller str,
    session_id: &'caller str,
    transport: &'caller str,
    user: &'caller str,
}

impl<'caller> Caller<'caller> {
    pub(crate) const fn new(
        transport: &'caller str,
        feature: &'caller str,
        user: &'caller str,
    ) -> Self {
        Self {
            feature,
            session_id: "",
            transport,
            user,
        }
    }

    pub(crate) const fn session(self, session_id: &'caller str) -> Self {
        Self { session_id, ..self }
    }
}

pub(crate) struct Outcome<'outcome> {
    attempt: u64,
    caller: Caller<'outcome>,
    code: Option<u16>,
    elapsed_us: u64,
    params: String,
    response: usize,
    status: String,
}

impl<'outcome> Outcome<'outcome> {
    pub(crate) fn attempt(self, attempt: u64) -> Self {
        Self { attempt, ..self }
    }

    pub(crate) fn code(self, code: Option<u16>) -> Self {
        Self { code, ..self }
    }

    pub(crate) const fn new(response: usize, status: String, elapsed_us: u64) -> Self {
        Self {
            attempt: 0,
            caller: Caller::new("", "", ""),
            code: None,
            elapsed_us,
            params: String::new(),
            response,
            status,
        }
    }

    pub(crate) fn params(self, params: String) -> Self {
        Self { params, ..self }
    }

    pub(crate) fn within(self, caller: Caller<'outcome>) -> Self {
        Self { caller, ..self }
    }
}

pub(crate) fn external(action: &str, request: usize, outcome: &Outcome<'_>) {
    let caller = outcome.caller;
    tracing::debug!(
        transport = caller.transport,
        session_id = caller.session_id,
        user = caller.user,
        feature = caller.feature,
        action,
        params = outcome.params,
        request = format!("redacted:{request}"),
        response = format!("redacted:{}", outcome.response),
        ok = outcome.status == "success",
        elapsed_us = outcome.elapsed_us,
        attempt = outcome.attempt,
        code = outcome.code,
        status = outcome.status,
    );
}

pub(crate) fn failure(done: &io::Result<Output>) -> String {
    done.as_ref().map_or_else(
        |err| format!("io:{:?}", err.kind()),
        |output| format!("exit:{}", output.status.code().unwrap_or_default()),
    )
}

pub(crate) fn finished(done: &io::Result<Output>) -> String {
    if done.as_ref().is_ok_and(|output| output.status.success()) {
        "success".to_owned()
    } else {
        failure(done)
    }
}
