use alloc::collections::BTreeMap;

use chrono::DateTime;
use schemars::JsonSchema;
use serde::Deserialize;

use super::prompt::{NoticeResponse, TIME_FORMAT};

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRemindRequest")]
pub(crate) struct RemindRequest {
    #[serde(default)]
    names: BTreeMap<String, String>,
    template: String,
    #[serde(default)]
    times: BTreeMap<String, u64>,
    #[serde(default)]
    values: BTreeMap<String, String>,
}

pub(crate) fn remind(req: RemindRequest) -> NoticeResponse {
    let RemindRequest {
        names,
        template,
        times,
        values,
    } = req;
    let named = values
        .iter()
        .map(|(key, value)| (key, names.get(value).unwrap_or(value).clone()));
    let stamped = times.iter().map(|(key, millis)| {
        let shown = i64::try_from(*millis)
            .ok()
            .and_then(DateTime::from_timestamp_millis)
            .map_or_default(|stamp| stamp.format(TIME_FORMAT).to_string());
        (key, shown)
    });
    let text = named.chain(stamped).fold(template, |done, (key, value)| {
        done.replace(&format!("{{{key}}}"), &value)
    });
    NoticeResponse::new(text)
}
