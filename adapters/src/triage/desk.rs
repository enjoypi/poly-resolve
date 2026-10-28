use domain::AppError;
use domain::features::{Call, Context, Origin, Target, dispatch};
use serde::Deserialize;
use serde_json::{Value, from_str, json};

use super::config::Settings;
use crate::config::Settings as Ingest;

const FEATURE: &str = "triage";
const TRANSPORT: &str = "adapters";

#[derive(Default, Deserialize)]
pub(crate) struct Ask {
    body: Option<String>,
    skip: Option<String>,
}

impl Ask {
    pub(crate) fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub(crate) fn skip(&self) -> &str {
        self.skip.as_deref().unwrap_or_default()
    }
}

#[derive(Default, Deserialize)]
struct Brief {
    prompt: String,
}

pub(crate) struct Desk<'desk> {
    context: &'desk Context<'desk>,
    ingest: &'desk Ingest,
    triage: &'desk Settings,
}

impl<'desk> Desk<'desk> {
    pub(crate) fn ask(&self, message: &Value) -> Ask {
        let triage = self.triage;
        let request = json!({
            "cipher_min_length": self.ingest.cipher_min_length(),
            "input_max_chars": triage.input_max_chars(),
            "message": message,
            "model": triage.model(),
            "notify_conversation_id": triage.notify_conversation_id().value().unwrap_or_default(),
            "question": {
                "instructions": triage.question().instructions(),
                "name": triage.question().name(),
            },
            "self_sender_id": triage.self_sender_id().value().unwrap_or_default(),
        });
        self.call("ask", &request)
            .map_or_default(|reply| from_str(&reply).unwrap_or_default())
    }

    pub(crate) fn brief(&self, rows: &[Value], trigger_ms: u64) -> String {
        let request = json!({
            "max_chars": self.triage.context_max_chars(),
            "rows": rows,
            "template": self.triage.prompt(),
            "trigger_ms": trigger_ms,
            "window_ms": self.triage.context_window_ms(),
        });
        self.call("brief", &request)
            .map_or_default(|reply| from_str::<Brief>(&reply).map_or_default(|brief| brief.prompt))
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

    pub(crate) fn notice(&self, handoff: &Value) -> String {
        let request = json!({
            "analysis": handoff.get("analysis"),
            "conversation_id": handoff.get("conversation_id"),
            "max_chars": self.triage.result_max_chars(),
            "probability": handoff.get("probability"),
            "template": self.triage.notice(),
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

    pub(crate) fn score(&self, body: &str) -> Option<Score> {
        let request = json!({
            "body": body,
            "name": self.triage.question().name(),
            "threshold": self.triage.threshold(),
        });
        let reply = self.call("score", &request).ok()?;
        from_str(&reply).ok()
    }
}

#[derive(Deserialize)]
pub(crate) struct Score {
    probability: f64,
    verdict: String,
}

impl Score {
    pub(crate) const fn probability(&self) -> f64 {
        self.probability
    }

    pub(crate) fn urgent(&self) -> bool {
        self.verdict == "urgent"
    }
}
