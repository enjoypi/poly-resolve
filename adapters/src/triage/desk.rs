use domain::AppError;
use domain::features::{Call, Context, Origin, Target, dispatch};
use serde::Deserialize;
use serde_json::{Map, Value, from_str, json, to_value};

use super::config::Settings;
use crate::config::Settings as Ingest;

const FEATURE: &str = "triage";
const RESUME: &str = "resume";
const TRANSPORT: &str = "adapters";

#[derive(Default, Deserialize)]
pub(crate) struct Brief {
    prompt: String,
    start: String,
    until_ms: u64,
}

impl Brief {
    pub(crate) fn prompt(&self) -> &str {
        &self.prompt
    }

    pub(crate) fn resumed(&self) -> bool {
        self.start == RESUME
    }

    pub(crate) const fn until_ms(&self) -> u64 {
        self.until_ms
    }
}

pub(crate) struct Desk<'desk> {
    context: &'desk Context<'desk>,
    ingest: &'desk Ingest,
    triage: &'desk Settings,
}

impl<'desk> Desk<'desk> {
    pub(crate) fn brief(&self, parts: &Value) -> Option<Brief> {
        let extra = json!({
            "followup": self.triage.followup(),
            "max_chars": self.triage.context_max_chars(),
            "template": self.triage.prompt(),
        });
        self.call("brief", &merged(parts, &extra))
            .map_or_default(|reply| from_str(&reply).ok())
    }

    fn call(&self, action: &str, request: &Value) -> Result<String, AppError> {
        dispatch(
            self.context,
            &Call::new(
                Origin::new(TRANSPORT, None, None),
                Target::new(FEATURE, action),
                &request.to_string(),
            ),
        )
    }

    pub(crate) fn line(&self, message: &Value, rules: &Value) -> Option<Line> {
        let triage = self.triage;
        let request = json!({
            "cipher_min_length": self.ingest.cipher_min_length(),
            "max_chars": triage.context_max_chars(),
            "message": message,
            "notify_conversation_id": triage.notify_conversation_id().value().unwrap_or_default(),
            "rules": rules,
            "self_sender_id": triage.self_sender_id().value().unwrap_or_default(),
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
    pub(crate) const fn new(
        context: &'desk Context<'desk>,
        ingest: &'desk Ingest,
        triage: &'desk Settings,
    ) -> Self {
        Self {
            context,
            ingest,
            triage,
        }
    }

    pub(crate) fn notice(&self, alert: &Value) -> String {
        let request = json!({
            "analysis": alert.get("analysis"),
            "max_chars": self.triage.result_max_chars(),
            "probability": alert.get("probability"),
            "template": self.triage.notice(),
            "topic_id": alert.get("topic_id"),
        });
        self.call("notice", &request).map_or_default(|reply| {
            from_str::<Value>(&reply).map_or_default(|notice| {
                notice
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
            "questions": {
                "topic": {
                    "instructions": questions.topic().instructions(),
                    "name": questions.topic().name(),
                },
                "urgent": {
                    "instructions": questions.urgent().instructions(),
                    "name": questions.urgent().name(),
                },
            },
        });
        self.call("pack", &merged(parts, &extra))
            .map_or_default(|reply| from_str(&reply).ok())
    }

    pub(crate) fn redact(&self, rules: &Value, text: &str) -> Result<String, AppError> {
        self.call("redact", &json!({"rules": rules, "text": text}))
    }

    pub(crate) fn reply(&self, stdout: &str) -> Option<Reply> {
        let request = json!({"max_chars": self.triage.result_max_chars(), "stdout": stdout});
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
            "threshold": questions.threshold(),
        });
        self.call("sort", &request)
            .map_or_default(|reply| from_str(&reply).ok())
    }
}

#[derive(Default, Deserialize)]
pub(crate) struct Line {
    sender: String,
    skip: Option<String>,
    text: String,
}

impl Line {
    pub(crate) fn sender(&self) -> &str {
        &self.sender
    }

    pub(crate) fn skip(&self) -> Option<&str> {
        self.skip.as_deref()
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Deserialize)]
pub(crate) struct Packed {
    body: String,
    layout: Value,
}

impl Packed {
    pub(crate) fn body(&self) -> &str {
        &self.body
    }

    pub(crate) const fn layout(&self) -> &Value {
        &self.layout
    }
}

#[derive(Deserialize)]
pub(crate) struct Reply {
    text: String,
    thread_id: String,
}

impl Reply {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn thread_id(&self) -> &str {
        &self.thread_id
    }
}

#[derive(Deserialize)]
pub(crate) struct Screened {
    flag: String,
    hints: Vec<String>,
    key: String,
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

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Deserialize)]
pub(crate) struct Sorted {
    placements: Vec<Value>,
    topics: Vec<Value>,
}

impl Sorted {
    pub(crate) fn placements(&self) -> &[Value] {
        &self.placements
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
