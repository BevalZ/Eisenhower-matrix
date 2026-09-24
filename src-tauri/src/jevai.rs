use serde_json::json;

use crate::models::*;

const API_URL: &str = "https://api.typesafe.ai/v1/systemone";

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

    let body = json!({
        "state": description,
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

    let client = reqwest::Client::new();
    let resp = client
        .post(API_URL)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status, text));
    }

    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let raw_importance = v["answers"]["importance"]["score"]
        .as_f64()
        .unwrap_or(2.5);
    let raw_urgency = v["answers"]["urgency"]["score"].as_f64().unwrap_or(2.5);

    // Apply user-calibrated bias from accumulated feedback
    let importance_score = (raw_importance + imp_bias).clamp(0.0, 4.0);
    let urgency_score = (raw_urgency + urg_bias).clamp(0.0, 4.0);

    let quadrant = scores_to_quadrant(importance_score, urgency_score);
    let priority = compute_priority(importance_score, urgency_score);

    Ok(ClassificationResult {
        quadrant,
        priority,
        importance_score,
        urgency_score,
        importance_label: label_for(importance_score, &IMPORTANCE_LABELS),
        urgency_label: label_for(urgency_score, &URGENCY_LABELS),
    })
}
