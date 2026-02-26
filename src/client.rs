use crate::error::{KrillError, Result};
use crate::types::{AutomationRequest, SseEvent};
use async_stream::stream;
use futures_util::{Stream, StreamExt};
use reqwest::header::CONTENT_TYPE;
use std::io::{Error as IoError, ErrorKind};

const RUN_SSE_URL: &str = "https://agent.tinyfish.ai/v1/automation/run-sse";

pub struct TinyFishClient {
    api_key: String,
    http: reqwest::Client,
}

impl TinyFishClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn run_sse(
        &self,
        payload: AutomationRequest,
    ) -> Result<impl Stream<Item = Result<SseEvent>>> {
        let response = self
            .http
            .post(RUN_SSE_URL)
            .header("X-API-Key", self.api_key.as_str())
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(KrillError::Http)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.map_err(KrillError::Http)?;
            return Err(KrillError::ApiStatus { status, body });
        }

        let sse_stream = stream! {
            let mut byte_stream = response.bytes_stream();
            let mut line_buffer: Vec<u8> = Vec::new();
            let mut event_name: Option<String> = None;
            let mut data_lines: Vec<String> = Vec::new();

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        yield Err(KrillError::Http(err));
                        return;
                    }
                };

                line_buffer.extend_from_slice(&chunk);

                while let Some(newline_idx) = line_buffer.iter().position(|&byte| byte == b'\n') {
                    let mut raw_line: Vec<u8> = line_buffer.drain(..=newline_idx).collect();
                    trim_newline_suffix(&mut raw_line);

                    let line = match String::from_utf8(raw_line) {
                        Ok(line) => line,
                        Err(err) => {
                            yield Err(KrillError::Io(IoError::new(ErrorKind::InvalidData, err)));
                            return;
                        }
                    };

                    if let Some(parsed) = parse_sse_line(&line, &mut event_name, &mut data_lines) {
                        let should_stop = parsed.is_err();
                        yield parsed;
                        if should_stop {
                            return;
                        }
                    }
                }
            }

            if !line_buffer.is_empty() {
                let line = match String::from_utf8(line_buffer) {
                    Ok(line) => line,
                    Err(err) => {
                        yield Err(KrillError::Io(IoError::new(ErrorKind::InvalidData, err)));
                        return;
                    }
                };

                if let Some(parsed) = parse_sse_line(&line, &mut event_name, &mut data_lines) {
                    let should_stop = parsed.is_err();
                    yield parsed;
                    if should_stop {
                        return;
                    }
                }
            }

            if event_name.is_some() || !data_lines.is_empty() {
                if data_lines.is_empty() {
                    yield Err(KrillError::SseUnexpectedEnd);
                    return;
                }

                let raw = data_lines.join("\n");
                yield parse_sse_payload(raw);
            }
        };

        Ok(sse_stream)
    }
}

fn parse_sse_line(
    line: &str,
    event_name: &mut Option<String>,
    data_lines: &mut Vec<String>,
) -> Option<Result<SseEvent>> {
    if line.is_empty() {
        if data_lines.is_empty() && event_name.is_none() {
            return None;
        }

        let raw = data_lines.join("\n");
        data_lines.clear();
        *event_name = None;

        if raw.is_empty() {
            return None;
        }

        return Some(parse_sse_payload(raw));
    }

    if line.starts_with(':') {
        return None;
    }

    if let Some(raw_event_name) = line.strip_prefix("event:") {
        *event_name = Some(raw_event_name.trim_start_matches(' ').to_owned());
        return None;
    }

    if let Some(raw_data) = line.strip_prefix("data:") {
        data_lines.push(raw_data.trim_start_matches(' ').to_owned());
        return None;
    }

    None
}

fn parse_sse_payload(raw: String) -> Result<SseEvent> {
    serde_json::from_str::<SseEvent>(&raw).map_err(|source| KrillError::SseParse { raw, source })
}

fn trim_newline_suffix(raw_line: &mut Vec<u8>) {
    if matches!(raw_line.last(), Some(b'\n')) {
        let _ = raw_line.pop();
    }
    if matches!(raw_line.last(), Some(b'\r')) {
        let _ = raw_line.pop();
    }
}
