use domain::AppError;
use domain::features::{Call, Context, Origin, Target, dispatch};
use domain::redact::Seal;
use serde::Deserialize;
use serde_json::{Map, Value, from_str, json, to_value};

use super::config::Settings;
use super::{number, text};
use crate::config::Settings as Ingest;
use crate::ingest::label;

const FEATURE: &str = "triage";
const PROMPTS: &str = "prompts";
const QUIET: &str = "quiet";
const RESUME: &str = "resume";
const TRANSPORT: &str = "adapters";

#[derive(Default, Deserialize)]
pub(crate) struct Brief {
    digest: String,
    prompt: String,
    start: String,
    version: String,
}

impl Brief {
    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }

    pub(crate) fn prompt(&self) -> &str {
        &self.prompt
    }

    pub(crate) fn resumed(&self) -> bool {
        self.start == RESUME
    }

    pub(crate) fn version(&self) -> &str {
        &self.version
    }
}

pub(crate) struct Desk<'desk> {
    context: &'desk Context<'desk>,
    ingest: &'desk Ingest,
    seal: Option<Seal>,
    triage: &'desk Settings,
}

impl<'desk> Desk<'desk> {
    fn ask(&self, target: Target<'_>, request: &Value) -> Result<String, AppError> {
        let context = self
            .seal
            .as_ref()
            .map_or(*self.context, |seal| self.context.with_seal(seal));
        dispatch(
            &context,
            &Call::new(
                Origin::new(TRANSPORT, None, None),
                target,
                &request.to_string(),
            ),
        )
    }

    pub(crate) fn brief(&self, parts: &Value) -> Option<Brief> {
        let extra = json!({
            "followup": self.prompt("followup")?,
            "max_chars": self.triage.context_max_chars(),
            "omitted": self.triage.omitted(),
            "template": self.prompt("session")?,
        });
        self.call("brief", &merged(parts, &extra))
            .map_or_default(|reply| from_str(&reply).ok())
    }

    fn call(&self, action: &str, request: &Value) -> Result<String, AppError> {
        self.ask(Target::new(FEATURE, action), request)
    }

    pub(crate) fn clues(&self, message: &Value) -> Option<Value> {
        let clues = self.triage.topics().clues();
        let request = json!({
            "message": {"raw": message.get("raw"), "text": text(message, "text")},
            "patterns": clues.patterns(),
            "subject": clues.subject(),
        });
        let found = self
            .call("clues", &request)
            .map_or_default(|reply| from_str::<Value>(&reply).ok())?;
        found.get("clues").cloned()
    }

    pub(crate) fn line(&self, message: &Value, rules: &Value) -> Option<Line> {
        let triage = self.triage;
        let profile = self
            .ingest
            .profile(&label(text(message, "source"), text(message, "account")));
        let request = json!({
            "cipher_min_length": profile.cipher_min_length(),
            "digest_min_chars": triage.digest_min_chars(),
            "max_chars": triage.context_max_chars(),
            "message": message,
            "notify_conversation_id": profile.notify_conversation_id(),
            "rules": rules,
            "self_sender_ids": profile.self_sender_ids(),
            "unknown_sender": triage.unknown_sender(),
        });
        self.call("line", &request)
            .map_or_default(|reply| from_str(&reply).ok())
    }

    pub(crate) fn masked(&self, rules: &Value, text: &str) -> String {
        self.redact(rules, text).map_or_default(|reply| {
            from_str::<Value>(&reply).map_or_default(|body| {
                body.get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            })
        })
    }

    #[must_use = "桌面需交给调度"]
    pub(crate) fn new(
        context: &'desk Context<'desk>,
        ingest: &'desk Ingest,
        triage: &'desk Settings,
    ) -> Self {
        Self {
            context,
            ingest,
            seal: triage.seal(),
            triage,
        }
    }

    pub(crate) fn notice(&self, alert: &Value) -> String {
        let notice = self.triage.notice();
        let late = number(alert, "created_at_ms").saturating_sub(number(alert, "last_at_ms"))
            > notice.late_after_ms();
        let request = json!({
            "analysis": alert.get("analysis"),
            "changes": alert.get("changes"),
            "label": alert.get("label"),
            "late": notice.late(),
            "late_ms": late.then(|| number(alert, "last_at_ms")),
            "max_chars": self.triage.result_max_chars(),
            "origin": alert.get("origin"),
            "origins": notice.origins(),
            "reach": alert.get("reach"),
            "score": alert.get("score"),
            "state": alert.get("state"),
            "states": notice.states(),
            "template": notice.template(),
            "topic_id": alert.get("topic_id"),
            "urgency": alert.get("urgency"),
        });
        self.call("notice", &request).map_or_default(|reply| {
            from_str::<Value>(&reply).map_or_default(|rendered| {
                rendered
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            })
        })
    }

    pub(crate) fn pack(&self, parts: &Value) -> Option<Packed> {
        let triage = self.triage;
        let questions = triage.questions();
        let extra = json!({
            "gap_ms": triage.session_gap_ms(),
            "label_chars": triage.label_chars(),
            "max_chars": triage.context_max_chars(),
            "model": triage.model(),
            "new_label": questions.topic().new_label(),
            "none_label": questions.topic().none_label(),
            "questions": {
                "review": {
                    "instructions": self.prompt("review")?,
                    "name": questions.review().name(),
                },
                "topic": {
                    "instructions": self.prompt("topic")?,
                    "name": questions.topic().name(),
                },
                "urgent": {
                    "instructions": self.prompt("urgent")?,
                    "name": questions.urgent().name(),
                },
            },
        });
        self.call("pack", &merged(parts, &extra))
            .map_or_default(|reply| from_str(&reply).ok())
    }

    pub(crate) fn people(&self, senders: &[Value]) -> Vec<Value> {
        self.call("people", &json!({"senders": senders}))
            .map_or_default(|reply| from_str::<Value>(&reply).unwrap_or_default())
            .get("identities")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn prompt(&self, name: &str) -> Option<String> {
        let text = self.triage.prompts().read(name)?;
        let request = json!({"name": name, "text": text});
        self.ask(Target::new(PROMPTS, "check"), &request)
            .map_err(|_err| tracing::warn!(name, len = text.len(), "prompt rejected"))
            .ok()
            .map(|_reply| text)
    }

    pub(crate) fn recap(&self, items: &[Value]) -> String {
        let notice = self.triage.notice();
        let request = json!({
            "item": notice.summary().item(),
            "items": items,
            "states": notice.states(),
            "template": notice.summary().template(),
        });
        self.call("recap", &request).map_or_default(|reply| {
            from_str::<Value>(&reply).map_or_default(|rendered| {
                rendered
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            })
        })
    }

    pub(crate) fn redact(&self, rules: &Value, text: &str) -> Result<String, AppError> {
        self.call("redact", &json!({"rules": rules, "text": text}))
    }

    pub(crate) fn regroup(&self, candidates: &[Value]) -> Vec<Value> {
        self.call("regroup", &json!({"candidates": candidates}))
            .map_or_default(|reply| from_str::<Value>(&reply).unwrap_or_default())
            .get("changes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn remind(&self, row: &Value) -> String {
        let alarm = self.triage.notice().alarm();
        let values: Map<String, Value> = ["account", "source", "state"]
            .into_iter()
            .map(|key| (key.to_owned(), Value::from(text(row, key))))
            .chain(
                ["streak", "tokens"]
                    .into_iter()
                    .map(|key| (key.to_owned(), Value::from(number(row, key).to_string()))),
            )
            .collect();
        let request = json!({
            "names": alarm.names(),
            "template": alarm.template(text(row, "kind")),
            "times": {"since": number(row, "since_ms")},
            "values": values,
        });
        self.call("remind", &request).map_or_default(|reply| {
            from_str::<Value>(&reply).map_or_default(|rendered| {
                rendered
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            })
        })
    }

    pub(crate) fn reply(&self, stdout: &str, state: &str) -> Option<Reply> {
        let request = json!({
            "max_chars": self.triage.result_max_chars(),
            "state": state,
            "stdout": stdout,
        });
        self.call("reply", &request)
            .map_or_default(|reply| from_str(&reply).ok())
    }

    pub(crate) fn rules(&self, learned: &[Value]) -> Value {
        let own = to_value(self.triage.rules()).unwrap_or_default();
        let mut all: Vec<Value> = learned
            .iter()
            .map(|rule| json!({"pattern": rule.get("pattern"), "placeholder": rule.get("placeholder")}))
            .collect();
        all.extend(own.as_array().cloned().unwrap_or_default());
        Value::Array(all)
    }

    pub(crate) fn screen(&self, request: &Value) -> String {
        self.call("screen", request).map_or_default(|reply| {
            from_str::<Value>(&reply).map_or_default(|found| {
                found
                    .get("body")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            })
        })
    }

    pub(crate) fn screened(&self, request: &Value) -> Option<Vec<Screened>> {
        self.call("screened", request)
            .map_or_default(|reply| from_str::<Verdicts>(&reply).ok())
            .map(|found| found.lines)
    }

    pub(crate) fn sort(&self, layout: &Value, response: &str) -> Option<Sorted> {
        let questions = self.triage.questions();
        let request = json!({
            "layout": layout,
            "min_confidence": questions.topic().min_confidence(),
            "response": response,
        });
        self.call("sort", &request)
            .map_or_default(|reply| from_str(&reply).ok())
    }

    pub(crate) fn spent(&self, text: &str) -> Value {
        self.call("spent", &json!({"text": text}))
            .map_or_default(|reply| from_str(&reply).unwrap_or_default())
    }

    pub(crate) fn started(&self, stdout: &str) -> Option<String> {
        let found: Value = self
            .call("started", &json!({"stdout": stdout}))
            .map_or_default(|reply| from_str(&reply).unwrap_or_default());
        found
            .get("thread_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
}

#[derive(Default, Deserialize)]
pub(crate) struct Line {
    digest: String,
    role: Option<String>,
    sender: String,
    skip: Option<String>,
    state: String,
    text: String,
}

impl Line {
    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }

    pub(crate) fn role(&self) -> &str {
        self.role.as_deref().unwrap_or_default()
    }

    pub(crate) fn sender(&self) -> &str {
        &self.sender
    }

    pub(crate) fn skip(&self) -> &str {
        self.skip.as_deref().unwrap_or_default()
    }

    pub(crate) fn state(&self) -> &str {
        &self.state
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Deserialize)]
pub(crate) struct Packed {
    body: String,
    layout: Value,
    questions: u64,
    version: String,
}

impl Packed {
    pub(crate) fn body(&self) -> &str {
        &self.body
    }

    pub(crate) const fn layout(&self) -> &Value {
        &self.layout
    }

    pub(crate) const fn questions(&self) -> u64 {
        self.questions
    }

    pub(crate) fn version(&self) -> &str {
        &self.version
    }
}

#[derive(Deserialize)]
pub(crate) struct Reply {
    changes: String,
    outcome: String,
    status: String,
    text: String,
    thread_id: String,
    title: String,
}

impl Reply {
    pub(crate) fn changes(&self) -> &str {
        &self.changes
    }

    pub(crate) fn quiet(&self) -> bool {
        self.outcome == QUIET
    }

    pub(crate) fn status(&self) -> &str {
        &self.status
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn thread_id(&self) -> &str {
        &self.thread_id
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }
}

#[derive(Default, Deserialize)]
pub(crate) struct Screened {
    flag: String,
    hints: Vec<String>,
    key: String,
    spans: Vec<String>,
    text: String,
}

impl Screened {
    pub(crate) fn flag(&self) -> &str {
        &self.flag
    }

    pub(crate) const fn hints(&self) -> &Vec<String> {
        &self.hints
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) const fn spans(&self) -> &Vec<String> {
        &self.spans
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Deserialize)]
pub(crate) struct Sorted {
    placements: Vec<Value>,
    relations: Vec<Value>,
    reviews: Vec<Value>,
    topics: Vec<Value>,
}

impl Sorted {
    pub(crate) fn placements(&self) -> &[Value] {
        &self.placements
    }

    pub(crate) fn relations(&self) -> &[Value] {
        &self.relations
    }

    pub(crate) fn reviews(&self) -> &[Value] {
        &self.reviews
    }

    pub(crate) fn topics(&self) -> &[Value] {
        &self.topics
    }
}

#[derive(Deserialize)]
struct Verdicts {
    lines: Vec<Screened>,
}

fn merged(parts: &Value, extra: &Value) -> Value {
    let mut map: Map<String, Value> = parts.as_object().cloned().unwrap_or_default();
    map.extend(extra.as_object().cloned().unwrap_or_default());
    Value::Object(map)
}
