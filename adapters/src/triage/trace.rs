pub(super) struct Outcome {
    attempt: u64,
    code: Option<u16>,
    elapsed_us: u64,
    response: usize,
    status: String,
}

impl Outcome {
    pub(super) fn attempt(self, attempt: u64) -> Self {
        Self { attempt, ..self }
    }

    pub(super) fn code(self, code: Option<u16>) -> Self {
        Self { code, ..self }
    }

    pub(super) const fn new(response: usize, status: String, elapsed_us: u64) -> Self {
        Self {
            attempt: 0,
            code: None,
            elapsed_us,
            response,
            status,
        }
    }
}

pub(super) fn external(action: &str, request: usize, outcome: &Outcome) {
    tracing::debug!(
        transport = "external",
        session_id = "",
        user = "",
        feature = "triage",
        action,
        params = "",
        request = format!("redacted:{request}"),
        response = format!("redacted:{}", outcome.response),
        ok = outcome.status == "success",
        elapsed_us = outcome.elapsed_us,
        attempt = outcome.attempt,
        code = outcome.code,
        status = outcome.status,
    );
}
