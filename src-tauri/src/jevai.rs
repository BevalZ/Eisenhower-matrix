use serde_json::json;

use crate::models::*;

const API_URL: &str = "https://api.typesafe.ai/v1/systemone";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub async fn classify(
    api_key: &str,
    description: &str,
    imp_bias: f64,
    urg_bias: f64,
) -> Result<ClassificationResult, String> {
    let importance_criteria: Vec<&str> = vec![
        "Trivial or irrelevant, no meaningful impact on any goal",
        "Minor impact, nice-to-have but not essential",
        "Moderate impact, supports some personal or work goals",
        "Significant impact, directly supports key personal or professional goals",
        "Critical, central to long-term success, health, or core responsibilities",
    ];
    let urgency_criteria: Vec<&str> = vec![
        "No deadline at all, can be done whenever",
        "Soft deadline, can comfortably wait weeks",
        "Reasonable deadline within the next few days",
        "Tight deadline within 24 to 48 hours",
        "Immediate — must be done right now or is already overdue",
    ];

    // Without today's date the model cannot tell how close "下周五" is.
    let today = chrono::Local::now().format("%Y-%m-%d (%A)");
    let state = format!(
        "Today is {today}. Judge any dates or deadlines relative to today.\n\n{description}"
    );

    let body = json!({
        "state": state,
        "model": "jev-latest",
        "questions": {
            "importance": {
                "type": "score",
                "instructions": "How important is this task to the user's long-term goals, career, health, or key responsibilities? Consider the task name, description, and the reasons the user gave for why it matters.",
                "criteria": importance_criteria
            },
            "urgency": {
                "type": "score",
                "instructions": "How urgent is this task? Consider whether it has a near-term deadline, whether delaying it causes immediate negative consequences, and the user's stated sense of time pressure.",
                "criteria": urgency_criteria
            }
        }
    });

    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("无法创建网络客户端: {}", e))?;
    let resp = client
        .post(API_URL)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "AI 服务响应超时，请稍后重试".to_string()
            } else {
                format!("请求失败: {}", e)
            }
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status, text));
    }

    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let (importance, urgency) = parse_scores(&v)?;
    Ok(classification(importance, urgency, imp_bias, urg_bias))
}

/// Shared by every provider: apply the learned bias, then map scores to a quadrant.
pub fn classification(importance: f64, urgency: f64, imp_bias: f64, urg_bias: f64) -> ClassificationResult {
    let importance_score = (importance + imp_bias).clamp(0.0, 4.0);
    let urgency_score = (urgency + urg_bias).clamp(0.0, 4.0);
    ClassificationResult {
        quadrant: scores_to_quadrant(importance_score, urgency_score),
        priority: compute_priority(importance_score, urgency_score),
        importance_score,
        urgency_score,
        importance_label: label_for(importance_score, &IMPORTANCE_LABELS),
        urgency_label: label_for(urgency_score, &URGENCY_LABELS),
    }
}


/// Read both scores from the API answer. A missing score is an error: falling back to a
/// default would silently file every task into the same quadrant if the API format changes.
fn parse_scores(v: &serde_json::Value) -> Result<(f64, f64), String> {
    let score = |key: &str, label: &str| {
        v["answers"][key]["score"]
            .as_f64()
            .filter(|s| s.is_finite())
            .ok_or_else(|| format!("AI 返回结果缺少{label}评分，请稍后重试或手动选择象限"))
    };
    Ok((score("importance", "重要性")?, score("urgency", "紧急性")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_scores() {
        let v = json!({ "answers": { "importance": { "score": 3 }, "urgency": { "score": 1.5 } } });
        assert_eq!(parse_scores(&v).unwrap(), (3.0, 1.5));
    }

    #[test]
    fn missing_or_invalid_scores_are_errors() {
        let missing = json!({ "answers": { "importance": { "score": 3 } } });
        assert!(parse_scores(&missing).unwrap_err().contains("紧急性"));
        let text =
            json!({ "answers": { "importance": { "score": "high" }, "urgency": { "score": 1 } } });
        assert!(parse_scores(&text).unwrap_err().contains("重要性"));
        assert!(parse_scores(&json!({ "error": "bad" })).is_err());
    }
}
