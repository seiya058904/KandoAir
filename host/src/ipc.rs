// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

//! The message protocol between the renderers (`src/common/host-bridge.ts`) and the host.
//!
//! Renderer to host (via `chrome.webview.postMessage`):
//!   `{ "k": "invoke", "id": 7, "ch": "common.general-settings-get", "args": [] }`
//!   `{ "k": "send", "ch": "common.log", "args": ["..."] }`
//!
//! Host to renderer (via `PostWebMessageAsJson`):
//!   `{ "k": "reply", "id": 7, "ok": true, "result": ... }`
//!   `{ "k": "reply", "id": 7, "ok": false, "error": "..." }`
//!   `{ "k": "event", "ch": "common.general-settings-changed", "args": [...] }`
//!
//! The channel names are the IPC channel names of Kando's Electron preload scripts.

use serde_json::{Value, json};

/// A parsed renderer message.
#[derive(Debug, PartialEq)]
pub struct Request {
    /// Set for invocations which expect a reply.
    pub id: Option<u64>,
    pub channel: String,
    pub args: Vec<Value>,
    /// Paths of files passed with `postMessageWithAdditionalObjects`.
    pub files: Vec<String>,
}

impl Request {
    pub fn arg(&self, index: usize) -> &Value {
        self.args.get(index).unwrap_or(&Value::Null)
    }

    pub fn str_arg(&self, index: usize) -> &str {
        self.arg(index).as_str().unwrap_or_default()
    }
}

pub fn parse(json: &str, files: Vec<String>) -> Option<Request> {
    let value: Value = serde_json::from_str(json).ok()?;
    let kind = value.get("k")?.as_str()?;
    let channel = value.get("ch")?.as_str()?.to_string();
    let args = value
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let id = match kind {
        "invoke" => Some(value.get("id")?.as_u64()?),
        "send" => None,
        _ => return None,
    };
    Some(Request {
        id,
        channel,
        args,
        files,
    })
}

pub fn reply(id: u64, result: Result<Value, String>) -> String {
    match result {
        Ok(value) => json!({ "k": "reply", "id": id, "ok": true, "result": value }).to_string(),
        Err(error) => json!({ "k": "reply", "id": id, "ok": false, "error": error }).to_string(),
    }
}

pub fn event(channel: &str, args: Vec<Value>) -> String {
    json!({ "k": "event", "ch": channel, "args": args }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_invocations_and_sends() {
        let r = parse(r#"{"k":"invoke","id":3,"ch":"a.b","args":[1,"x"]}"#, vec![]).unwrap();
        assert_eq!(r.id, Some(3));
        assert_eq!(r.channel, "a.b");
        assert_eq!(r.str_arg(1), "x");
        assert_eq!(r.arg(5), &Value::Null);
        let s = parse(r#"{"k":"send","ch":"c","args":[]}"#, vec![]).unwrap();
        assert_eq!(s.id, None);
        assert!(parse(r#"{"k":"invoke","ch":"no-id"}"#, vec![]).is_none());
        assert!(parse("not json", vec![]).is_none());
    }

    #[test]
    fn formats_replies_and_events() {
        assert_eq!(
            reply(1, Ok(json!(5))),
            r#"{"id":1,"k":"reply","ok":true,"result":5}"#
        );
        assert_eq!(
            reply(2, Err("x".into())),
            r#"{"error":"x","id":2,"k":"reply","ok":false}"#
        );
        assert_eq!(
            event("e", vec![json!(true)]),
            r#"{"args":[true],"ch":"e","k":"event"}"#
        );
    }
}
