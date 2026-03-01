use serde_json::json;

const OPENAI_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";
const ANALYSIS_SYSTEM_PROMPT: &str = "You are an elite quantitative financial analyst. Review the provided scraped JSON data from various web sources. Synthesize the data, identify the top 3 macro market trends, note any conflicting reports, and output a highly structured, professional Markdown report. Do not include pleasantries.";

pub async fn synthesize_report(
    aggregated_data: &str,
    api_key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let payload = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {
                "role": "system",
                "content": ANALYSIS_SYSTEM_PROMPT
            },
            {
                "role": "user",
                "content": aggregated_data
            }
        ]
    });

    let client = reqwest::Client::new();
    let response = client
        .post(OPENAI_CHAT_COMPLETIONS_URL)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI request failed ({status}): {body}").into());
    }

    let response_json: serde_json::Value = response.json().await?;
    let report = response_json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .ok_or("OpenAI response did not include choices[0].message.content")?;

    Ok(report.to_owned())
}
