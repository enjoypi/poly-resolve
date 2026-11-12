use domain::batches::{Item as Batch, Roster};
use domain::features::{Call, Context, Origin, Ports, Rules, Target, dispatch};
use domain::inbox::{Paging, Query, Reader, Row};
use domain::outbound::{Item, Listing, Outbox};
use domain::prompts::Library;
use domain::redact::Seal;
use domain::{AppError, Errors};
use serde_json::{Error, from_str, from_value, json};

const BATCH: u64 = 1;
pub(crate) const ERRORS: &str = r#"{"bad_request":{"code":"invalid"},"conflict":{"code":"clash"},"not_found":{"code":"absent"},"storage":{"code":"unavailable"},"unknown_action":{"code":"missing"},"unprocessable":{"code":"refused"}}"#;
const HASH_CHARS: u32 = 8;
pub(crate) const MARK: &str = "[x]";
pub(crate) const PAGING: &str = r#"{"default":2,"max":3}"#;
const STAMP: u64 = 1_000;

#[derive(Debug)]
pub(crate) struct Broken;

impl Outbox for Broken {
    fn list(&self, _listing: &Listing<'_>) -> Result<Vec<Item>, AppError> {
        Err(AppError::Storage)
    }
}

impl Reader for Broken {
    fn page(&self, _query: &Query) -> Result<Vec<Row>, AppError> {
        Err(AppError::Storage)
    }
}

impl Roster for Broken {
    fn list(&self, _listing: &Listing<'_>) -> Result<Vec<Batch>, AppError> {
        Err(AppError::Storage)
    }

    fn revive(&self, _batch_id: u64) -> Result<Option<String>, AppError> {
        Err(AppError::Storage)
    }
}

pub(crate) struct Fixture {
    errors: Errors,
    paging: Paging,
    placeholders: Vec<String>,
    seal: Seal,
}

impl Fixture {
    pub(crate) fn dispatch(&self, call: &Call<'_>) -> Result<String, AppError> {
        self.dispatch_with(&Rows::default(), call)
    }

    fn dispatch_both(&self, ports: Ports<'_>, call: &Call<'_>) -> Result<String, AppError> {
        let rules = Rules::new(&self.paging).with_placeholders(&self.placeholders);
        dispatch(
            &Context::new(&self.errors, rules, ports).with_seal(&self.seal),
            call,
        )
    }

    pub(crate) fn dispatch_outbox(
        &self,
        outbox: &dyn Outbox,
        call: &Call<'_>,
    ) -> Result<String, AppError> {
        self.dispatch_both(Ports::new(&Rows::default(), outbox, &Pool::default()), call)
    }

    pub(crate) fn dispatch_prompts(
        &self,
        library: &dyn Library,
        call: &Call<'_>,
    ) -> Result<String, AppError> {
        let (reader, outbox, roster) = (Rows::default(), Sent::default(), Pool::default());
        self.dispatch_both(
            Ports::new(&reader, &outbox, &roster).with_library(library),
            call,
        )
    }

    pub(crate) fn dispatch_roster(
        &self,
        roster: &dyn Roster,
        call: &Call<'_>,
    ) -> Result<String, AppError> {
        self.dispatch_both(Ports::new(&Rows::default(), &Sent::default(), roster), call)
    }

    pub(crate) fn dispatch_unsealed(&self, call: &Call<'_>) -> Result<String, AppError> {
        let rules = Rules::new(&self.paging);
        let (reader, outbox, roster) = (Rows::default(), Sent::default(), Pool::default());
        dispatch(
            &Context::new(&self.errors, rules, Ports::new(&reader, &outbox, &roster)),
            call,
        )
    }

    pub(crate) fn dispatch_with(
        &self,
        reader: &dyn Reader,
        call: &Call<'_>,
    ) -> Result<String, AppError> {
        self.dispatch_both(Ports::new(reader, &Sent::default(), &Pool::default()), call)
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
            placeholders: vec![MARK.to_owned()],
            seal: Seal::new("test material", HASH_CHARS),
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Pool(Vec<(u64, String)>);

impl Pool {
    pub(crate) fn new(items: &[(u64, &str)]) -> Self {
        Self(
            items
                .iter()
                .map(|&(batch_id, state)| (batch_id, state.to_owned()))
                .collect(),
        )
    }
}

impl Roster for Pool {
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Batch>, AppError> {
        let limit = usize::try_from(listing.limit()).unwrap_or(usize::MAX);
        self.0
            .iter()
            .filter(|entry| entry.1 == listing.state())
            .take(limit)
            .map(|entry| {
                let (batch_id, state) = (entry.0, &entry.1);
                from_value(json!({
                    "attempts": BATCH,
                    "batch_id": batch_id,
                    "lines": BATCH,
                    "reason": "http:500",
                    "state": state,
                    "updated_at_ms": STAMP,
                }))
                .map_err(|_err| AppError::Storage)
            })
            .collect()
    }

    fn revive(&self, batch_id: u64) -> Result<Option<String>, AppError> {
        Ok(self
            .0
            .iter()
            .find(|entry| entry.0 == batch_id)
            .map(|entry| {
                if entry.1 == "failed" {
                    "ready".to_owned()
                } else {
                    entry.1.clone()
                }
            }))
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

#[derive(Debug, Default)]
pub(crate) struct Sent(Vec<(String, String)>);

impl Sent {
    pub(crate) fn new(items: &[(&str, &str)]) -> Self {
        Self(
            items
                .iter()
                .map(|&(key, state)| (key.to_owned(), state.to_owned()))
                .collect(),
        )
    }
}

impl Outbox for Sent {
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Item>, AppError> {
        let limit = usize::try_from(listing.limit()).unwrap_or(usize::MAX);
        self.0
            .iter()
            .filter(|entry| entry.1 == listing.state())
            .take(limit)
            .map(|entry| {
                let (key, state) = (&entry.0, &entry.1);
                from_value(json!({
                    "account": "default",
                    "batch_id": BATCH,
                    "conversation_id": "c1",
                    "flag": "",
                    "key": key,
                    "occurred_at_ms": STAMP,
                    "reason": "",
                    "role": "",
                    "sender": "alice",
                    "source": "dingtalk",
                    "state": state,
                    "text": "hello",
                }))
                .map_err(|_err| AppError::Storage)
            })
            .collect()
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
