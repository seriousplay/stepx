//! Separate the desktop tool envelope from the human-readable answer.
//! A final answer may legitimately follow the envelope in the same message.
use anyhow::{bail, Context, Result};
use serde_json::Value;

const OPEN: &str = "<amoo_work_protocol>";
const CLOSE: &str = "</amoo_work_protocol>";

pub struct WorkResponse {
    pub plan: Value,
    pub body: String,
}

fn unfence(text: &str) -> &str {
    let text = text.trim();
    if let Some(body) = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
    {
        if let Some(body) = body.strip_suffix("```") {
            return body.trim();
        }
    }
    text
}

/// Exactly one complete tool envelope is accepted. A malformed or ambiguous
/// envelope must never lead to execution or be treated as successful completion.
pub fn parse_response(text: &str) -> Result<Option<WorkResponse>> {
    let raw = unfence(text);
    if let Some(start) = raw.find(OPEN) {
        let inner_start = start + OPEN.len();
        let end = raw[inner_start..]
            .find(CLOSE)
            .map(|index| inner_start + index)
            .context("桌面执行协议不完整，已停止执行；未将其当作完成回执")?;
        let before = raw[..start].trim();
        let after = raw[end + CLOSE.len()..].trim();
        if raw[inner_start..end].contains(OPEN)
            || after.contains(OPEN)
            || after.contains(CLOSE)
            || before.contains(CLOSE)
        {
            bail!("手机返回了多个或嵌套的桌面执行协议，已停止执行");
        }
        let plan: Value = serde_json::from_str(unfence(&raw[inner_start..end]))
            .context("桌面执行协议不是有效 JSON，已停止执行；已有文件已保留")?;
        if plan.get("amoo_work").is_none() {
            bail!("桌面执行协议缺少版本，已停止执行");
        }
        let body = [before, after]
            .into_iter()
            .filter(|part| !part.is_empty() && *part != "```" && *part != "```json")
            .collect::<Vec<_>>()
            .join("\n\n");
        return Ok(Some(WorkResponse { plan, body }));
    }
    if raw.contains(CLOSE) {
        bail!("桌面执行协议缺少起始标记，已停止执行");
    }
    // Older replies contain a bare JSON object, optionally in a code fence.
    // Ordinary prose and JSON examples without the protocol key stay untouched.
    if raw.starts_with('{') || raw.starts_with("```json") {
        let candidate = raw.strip_prefix("```json").unwrap_or(raw).trim();
        let mut values = serde_json::Deserializer::from_str(candidate).into_iter::<Value>();
        match values.next() {
            Some(Ok(plan)) if plan.get("amoo_work").is_some() => {
                let trailing = candidate[values.byte_offset()..].trim();
                let body = trailing
                    .strip_prefix("```")
                    .unwrap_or(trailing)
                    .trim()
                    .to_string();
                if body.contains("\"amoo_work\"") {
                    bail!("手机返回了多个桌面执行计划，已停止执行");
                }
                return Ok(Some(WorkResponse { plan, body }));
            }
            Some(Err(_)) if raw.contains("\"amoo_work\"") => {
                bail!("桌面执行协议不是有效 JSON，已停止执行")
            }
            _ => {}
        }
    }
    Ok(None)
}

/// Presentation is tolerant: keep the actual answer, but never show protocol
/// wrappers or internal desktop result payloads as chat prose.
pub fn display_text(text: &str, user: bool) -> Option<String> {
    let mut clean = text.to_string();
    if user {
        for tag in [
            "amoo_work_image_instructions",
            "amoo_work_upload_instructions",
        ] {
            let open = format!("<{tag}>");
            let close = format!("</{tag}>");
            if let Some(start) = clean.find(&open) {
                if let Some(end) = clean[start + open.len()..].find(&close) {
                    clean.replace_range(start..start + open.len() + end + close.len(), "");
                }
            }
        }
    }
    let raw = clean.trim();
    if user {
        if let Some(task) =
            raw.strip_prefix("<amoo_work_protocol>\n[Amoo Work 桌面协作]\n用户任务：")
        {
            return Some(
                task.split_once("\n用户已选择一个本地工作目录。")
                    .map_or(task, |(task, _)| task)
                    .trim()
                    .to_string(),
            );
        }
        if raw.starts_with("<amoo_work_protocol>\nAmoo Work 桌面执行结果") {
            return None;
        }
        return Some(raw.to_string());
    }
    match parse_response(text) {
        Ok(Some(response)) => {
            if !response.body.trim().is_empty() {
                return Some(response.body);
            }
            response.plan["summary"]
                .as_str()
                .filter(|summary| !summary.trim().is_empty())
                .map(str::to_string)
        }
        Ok(None) => Some(text.to_string()),
        Err(_) => {
            // Preserve surrounding prose even if a protocol block was broken.
            let mut body = String::new();
            let mut rest = raw;
            while let Some(start) = rest.find(OPEN) {
                body.push_str(&rest[..start]);
                let tail = &rest[start + OPEN.len()..];
                if let Some(end) = tail.find(CLOSE) {
                    rest = &tail[end + CLOSE.len()..];
                } else {
                    rest = "";
                    break;
                }
            }
            body.push_str(rest);
            let body = body.trim();
            if body.is_empty() || body.contains("\"amoo_work\"") || body.contains(CLOSE) {
                None
            } else {
                Some(body.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_plan_followed_by_formatted_answer_completes_and_keeps_answer() {
        let text = "<amoo_work_protocol>\n{\"amoo_work\":1,\"summary\":\"分析完成\",\"done\":true,\"tools\":[]}\n</amoo_work_protocol>\n\n**交付文件**\n- `报告.docx`\n\n| 项目 | 合计 |\n|---|---|\n| A | 43 |";
        let reply = parse_response(text).unwrap().unwrap();
        assert_eq!(reply.plan["done"], true);
        let shown = display_text(text, false).unwrap();
        assert!(shown.starts_with("**交付文件**"));
        assert!(shown.contains("| A | 43 |"));
        assert!(!shown.contains("amoo_work"));
    }
    #[test]
    fn protocol_fences_and_legacy_json_are_supported() {
        for text in [
            "```json\n{\"amoo_work\":1,\"summary\":\"完成\",\"done\":true}\n```",
            "<amoo_work_protocol>```json\n{\"amoo_work\":1,\"summary\":\"完成\",\"done\":true}\n```</amoo_work_protocol>",
            "```json\n{\"amoo_work\":1,\"done\":true}\n```\n完成",
        ] { assert_eq!(display_text(text, false).as_deref(), Some("完成")); }
    }
    #[test]
    fn broken_or_multiple_plans_never_execute_but_keep_surrounding_answer() {
        let broken = "<amoo_work_protocol>broken</amoo_work_protocol>\n已保存的文件仍可打开。";
        assert!(parse_response(broken).is_err());
        assert_eq!(
            display_text(broken, false).as_deref(),
            Some("已保存的文件仍可打开。")
        );
        let duplicate = "<amoo_work_protocol>{\"amoo_work\":1}</amoo_work_protocol><amoo_work_protocol>{\"amoo_work\":1}</amoo_work_protocol>";
        assert!(parse_response(duplicate).is_err());
        assert!(display_text(duplicate, false).is_none());
        assert!(parse_response("<amoo_work_protocol>{").is_err());
    }
    #[test]
    fn human_json_and_user_requests_are_not_hidden() {
        let text = "请解释 `amoo_work` 的含义和 {\"done\":true}。";
        assert_eq!(display_text(text, false).as_deref(), Some(text));
        assert_eq!(display_text(text, true).as_deref(), Some(text));
        assert!(parse_response("{\"sales\":43}").unwrap().is_none());
        assert_eq!(display_text("蓝色纸船\n\n<amoo_work_image_instructions>Use the actual phone tool.</amoo_work_image_instructions>",true).as_deref(),Some("蓝色纸船"));
    }
}
