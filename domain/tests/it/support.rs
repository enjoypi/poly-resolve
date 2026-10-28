use domain::features::{Call, Context, Origin, Target, dispatch};
use domain::inbox::{Paging, Query, Reader, Row};
use domain::{AppError, Errors};
use serde_json::{Error, from_str, from_value, json};

pub(crate) const ERRORS: &str = r#"{"bad_request":{"code":"invalid","exit_code":13},"storage":{"code":"unavailable","exit_code":15},"unknown_action":{"code":"missing","exit_code":14}}"#;
pub(crate) const PAGING: &str = r#"{"default":2,"max":3}"#;

#[derive(Debug)]
pub(crate) struct Broken;

impl Reader for Broken {
    fn page(&self, _query: &Query) -> Result<Vec<Row>, AppError> {
        Err(AppError::Storage)
    }
}

pub(crate) struct Fixture {
    errors: Errors,
    paging: Paging,
}

impl Fixture {
    pub(crate) fn dispatch(&self, call: &Call<'_>) -> Result<String, AppError> {
        self.dispatch_with(&Rows::default(), call)
    }

    pub(crate) fn dispatch_with(
        &self,
        reader: &dyn Reader,
        call: &Call<'_>,
    ) -> Result<String, AppError> {
        dispatch(&Context::new(&self.errors, &self.paging, reader), call)
    }

    pub(crate) const fn errors(&self) -> &Errors {
        &self.errors
    }

    pub(crate) fn new() -> Result<Self, Error> {
        Self::with(ERRORS)
    }

    pub(crate) fn with(errors: &str) -> Result<Self, Error> {
        Ok(Self {
            errors: from_str(errors)?,
            paging: from_str(PAGING)?,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Rows(Vec<Row>);

impl Rows {
    pub(crate) const fn new(rows: Vec<Row>) -> Self {
        Self(rows)
    }
}

impl Reader for Rows {
    fn page(&self, query: &Query) -> Result<Vec<Row>, AppError> {
        let limit = usize::try_from(query.limit()).unwrap_or(usize::MAX);
        Ok(self
            .0
            .iter()
            .filter(|row| {
                query.cursor().is_none_or(|mark| {
                    (row.occurred_at_ms(), row.key()) < (mark.occurred_at_ms(), mark.key())
                })
            })
            .filter(|row| {
                let window = query.window();
                window
                    .since()
                    .is_none_or(|since| row.occurred_at_ms() >= since)
                    && window
                        .until()
                        .is_none_or(|until| row.occurred_at_ms() < until)
            })
            .take(limit)
            .cloned()
            .collect())
    }
}

pub(crate) const fn cli<'call>(
    feature: &'call str,
    action: &'call str,
    request: &'call str,
) -> Call<'call> {
    Call::new(
        Origin::new("cli", None, None),
        Target::new(feature, action),
        request,
    )
}

pub(crate) fn row(key: &str, occurred_at_ms: u64) -> Result<Row, Error> {
    from_value(json!({
        "account": "acct",
        "conversation_id": "c1",
        "key": key,
        "occurred_at_ms": occurred_at_ms,
        "received_at_ms": occurred_at_ms,
        "sender_id": "u1",
        "sender_name": "alice",
        "source": "dingtalk",
        "text": "hello",
    }))
}
