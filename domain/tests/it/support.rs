use domain::features::{Call, Context, Origin, Ports, Rules, Target, dispatch};
use domain::inbox::{Paging, Query, Reader, Row};
use domain::review::{Docket, Item, Listing, Reason, Ruling};
use domain::{AppError, Errors};
use serde_json::{Error, from_str, from_value, json};

const BATCH: u64 = 1;
pub(crate) const ERRORS: &str = r#"{"bad_request":{"code":"invalid"},"conflict":{"code":"clash"},"not_found":{"code":"absent"},"storage":{"code":"unavailable"},"unknown_action":{"code":"missing"},"unprocessable":{"code":"refused"}}"#;
pub(crate) const LEAKED: &str = "the secret-token-123 here";
const LEARNED: &str = r"secret\-token\-123";
pub(crate) const LEARN_MIN: u32 = 4;
pub(crate) const PAGING: &str = r#"{"default":2,"max":3}"#;
pub(crate) const REASONS: &str =
    r#"[{"code":"leak","label":"Leaked","placeholder":"[x]"},{"code":"noise","label":"Noise"}]"#;
const STAMP: u64 = 1_000;

#[derive(Debug)]
pub(crate) struct Broken;

impl Docket for Broken {
    fn list(&self, _listing: &Listing<'_>) -> Result<Vec<Item>, AppError> {
        Err(AppError::Storage)
    }

    fn settle(&self, _ruling: &Ruling<'_>) -> Result<Option<String>, AppError> {
        Err(AppError::Storage)
    }

    fn source(&self, _key: &str) -> Result<Option<String>, AppError> {
        Err(AppError::Storage)
    }
}

impl Reader for Broken {
    fn page(&self, _query: &Query) -> Result<Vec<Row>, AppError> {
        Err(AppError::Storage)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Dock(Vec<Entry>);

impl Dock {
    pub(crate) fn new(items: &[(&str, &str)]) -> Self {
        Self(
            items
                .iter()
                .map(|&(key, state)| Entry {
                    key: key.to_owned(),
                    state: state.to_owned(),
                })
                .collect(),
        )
    }
}

impl Docket for Dock {
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Item>, AppError> {
        let limit = usize::try_from(listing.limit()).unwrap_or(usize::MAX);
        self.0
            .iter()
            .filter(|entry| entry.state == listing.state())
            .take(limit)
            .map(|entry| {
                from_value(json!({
                    "batch_id": BATCH,
                    "conversation_id": "c1",
                    "flag": "",
                    "key": entry.key,
                    "occurred_at_ms": STAMP,
                    "reason": "",
                    "sender": "alice",
                    "state": entry.state,
                    "text": LEAKED,
                }))
                .map_err(|_err| AppError::Storage)
            })
            .collect()
    }

    fn settle(&self, ruling: &Ruling<'_>) -> Result<Option<String>, AppError> {
        Ok(self
            .0
            .iter()
            .find(|entry| entry.key == ruling.key())
            .map(|entry| {
                if entry.state == "pending" {
                    ruling
                        .rule()
                        .filter(|rule| (rule.pattern(), rule.placeholder()) != (LEARNED, "[x]"))
                        .map_or_else(|| ruling.state(), |_| "mislearned")
                        .to_owned()
                } else {
                    entry.state.clone()
                }
            }))
    }

    fn source(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .0
            .iter()
            .find(|entry| entry.key == key)
            .map(|_| LEAKED.to_owned()))
    }
}

#[derive(Debug)]
struct Entry {
    key: String,
    state: String,
}

pub(crate) struct Fixture {
    errors: Errors,
    paging: Paging,
    reasons: Vec<Reason>,
}

impl Fixture {
    pub(crate) fn dispatch(&self, call: &Call<'_>) -> Result<String, AppError> {
        self.dispatch_with(&Rows::default(), call)
    }

    fn dispatch_both(&self, ports: Ports<'_>, call: &Call<'_>) -> Result<String, AppError> {
        let rules = Rules::new(&self.paging, &self.reasons, LEARN_MIN);
        dispatch(&Context::new(&self.errors, rules, ports), call)
    }

    pub(crate) fn dispatch_review(
        &self,
        docket: &dyn Docket,
        call: &Call<'_>,
    ) -> Result<String, AppError> {
        self.dispatch_both(Ports::new(&Rows::default(), docket), call)
    }

    pub(crate) fn dispatch_with(
        &self,
        reader: &dyn Reader,
        call: &Call<'_>,
    ) -> Result<String, AppError> {
        self.dispatch_both(Ports::new(reader, &Dock::default()), call)
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
            reasons: from_str(REASONS)?,
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

pub(crate) const fn local<'call>(
    feature: &'call str,
    action: &'call str,
    request: &'call str,
) -> Call<'call> {
    Call::new(
        Origin::new("adapters", None, None),
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
