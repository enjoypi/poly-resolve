use serde_json::{Value, json};

use crate::support::{FRAGMENTS, SEALED};

pub(crate) fn barred(ident: &str) -> String {
    json!({
        "type": "user_im_message_receive_o2o_all",
        "message_id": ident,
        "conversation_id": "c1",
        "sender": "alice",
        "sender_open_dingtalk_id": "u1",
        "content": "a || b",
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
}

pub(crate) fn event(ident: &str) -> String {
    json!({
        "type": "user_im_message_receive_o2o_all",
        "message_id": ident,
        "conversation_id": "c1",
        "sender": "alice",
        "sender_open_dingtalk_id": "u1",
        "content": "hello",
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
}

pub(crate) fn feed(lines: &[String]) -> String {
    format!("{}\n", lines.join("\n"))
}

pub(crate) fn fragment_event(ident: &str) -> String {
    json!({
        "type": "user_im_message_receive_group_all",
        "message_id": ident,
        "conversation_id": "c1",
        "sender": "alice",
        "sender_open_dingtalk_id": "u1",
        "content": FRAGMENTS,
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
}

pub(crate) fn fragment_page(ids: &[&str]) -> String {
    let messages: Vec<Value> = ids
        .iter()
        .map(|ident| {
            json!({
                "messageId": ident,
                "sender": "bob",
                "content": if ident.starts_with('p') { "plain" } else { FRAGMENTS },
                "createTime": "2026-07-08 12:00:35",
            })
        })
        .collect();
    json!({
        "result": {
            "conversationMessagesList": [{"openConversationId": "c1", "messages": messages}],
            "nextCursor": null,
        }
    })
    .to_string()
}

pub(crate) fn page(ids: &[&str], cursor: Option<&str>) -> String {
    let messages: Vec<Value> = ids
        .iter()
        .map(|ident| {
            json!({
                "messageId": ident,
                "sender": "bob",
                "text": "yo",
                "createTime": "2026-07-08 12:00:35",
            })
        })
        .collect();
    json!({
        "result": {
            "conversationMessagesList": [{"openConversationId": "c1", "messages": messages}],
            "nextCursor": cursor,
        }
    })
    .to_string()
}

pub(crate) fn sealed(ident: &str) -> String {
    json!({
        "type": "user_im_message_receive_o2o_all",
        "message_id": ident,
        "conversation_id": "c1",
        "sender": "alice",
        "sender_open_dingtalk_id": "u1",
        "content": SEALED,
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
}

pub(crate) fn sealed_page(ids: &[&str]) -> String {
    let messages: Vec<Value> = ids
        .iter()
        .map(|ident| {
            json!({
                "messageId": ident,
                "sender": "bob",
                "text": SEALED,
                "createTime": "2026-07-08 12:00:35",
            })
        })
        .collect();
    json!({
        "result": {
            "conversationMessagesList": [{"openConversationId": "c1", "messages": messages}],
            "nextCursor": null,
        }
    })
    .to_string()
}
