//! OpenAI-compatible chat completions (OpenAI, DeepSeek, Qwen, Moonshot, local Ollama …).

use serde_json::{json, Value};

use crate::jevai::classification;
use crate::models::*;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

const PROMPT: &str = "You rate tasks for an Eisenhower matrix. Reply with JSON only: \
{\"importance\": <0-4>, \"urgency\": <0-4>}. \
importance: 0 trivial, 1 minor, 2 moderate, 3 significant for key goals, 4 critical. \
urgency: 0 no deadline, 1 can wait weeks, 2 due in a few days, 3 due within 24-48 hours, \
4 must be done now or overdue. The task may be written in Chinese.";

pub async fn classify(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    description: &str,
    imp_bias: f64,
    urg_bias: f64,
) -> Result<ClassificationResult, String> {
    let url = format!("{}/chat/completions", base_url.trim().trim_end_matches('/'));
    let today = chrono::Local::now().format("%Y-%m-%d (%A)");
    let body = json!({
        "model": model,
        "temperature": 0,
        "messages": [
            { "role": "system", "content": PROMPT },
            { "role": "user", "content": format!("Today is {today}.\n\n{description}") }
        ]
    });
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("无法创建网络客户端: {e}"))?;
    let mut req = client.post(&url).json(&body);
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() {
            "AI 服务响应超时，请稍后重试".to_string()
        } else {
            format!("请求失败: {e}")
        }
    })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {status}: {}", text.chars().take(300).collect::<String>()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("AI 返回内容为空")?;
    let (importance, urgency) = parse_reply(content)?;
    Ok(classification(importance, urgency, imp_bias, urg_bias))
}

/// Pull the scores out of the model reply; tolerates code fences and extra text.
fn parse_reply(content: &str) -> Result<(f64, f64), String> {
    let start = content.find('{');
    let end = content.rfind('}');
    let json = match (start, end) {
        (Some(s), Some(e)) if s < e => &content[s..=e],
        _ => return Err("AI 没有返回评分，请换一个模型或手动选择象限".into()),
    };
    let v: Value = serde_json::from_str(json).map_err(|_| "AI 返回的评分格式无法解析".to_string())?;
    let score = |key: &str, label: &str| {
        v[key]
            .as_f64()
            .filter(|s| s.is_finite())
            .map(|s| s.clamp(0.0, 4.0))
            .ok_or_else(|| format!("AI 返回结果缺少{label}评分"))
    };
    Ok((score("importance", "重要性")?, score("urgency", "紧急性")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_fenced_replies() {
        assert_eq!(parse_reply(r#"{"importance": 3, "urgency": 1}"#).unwrap(), (3.0, 1.0));
        let fenced = "```json\n{\"importance\": 2.5, \"urgency\": 9}\n```";
        assert_eq!(parse_reply(fenced).unwrap(), (2.5, 4.0));
    }

    #[test]
    fn rejects_replies_without_scores() {
        assert!(parse_reply("I think it is important").is_err());
        assert!(parse_reply(r#"{"importance": 3}"#).is_err());
    }
}
