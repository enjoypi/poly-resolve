use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str};

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageSpentRequest")]
pub(crate) struct SpentRequest {
    text: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriageUsage")]
pub(crate) struct Usage {
    #[serde(default, rename = "input_tokens")]
    input: u64,
    #[serde(default, rename = "output_tokens")]
    output: u64,
}

impl Usage {
    const fn plus(self, other: Self) -> Self {
        Self {
            input: self.input.saturating_add(other.input),
            output: self.output.saturating_add(other.output),
        }
    }
}

fn found(value: &Value) -> Usage {
    value
        .get("usage")
        .and_then(|usage| Usage::deserialize(usage).ok())
        .unwrap_or_default()
}

pub(crate) fn spent(req: SpentRequest) -> Usage {
    let SpentRequest { text } = req;
    from_str::<Value>(&text).map_or_else(
        |_err| {
            text.lines()
                .filter_map(|line| from_str::<Value>(line).ok())
                .fold(Usage::default(), |sum, event| sum.plus(found(&event)))
        },
        |whole| found(&whole),
    )
}
