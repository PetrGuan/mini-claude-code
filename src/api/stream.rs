use crate::api::types::StreamEvent;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::Response;
use tokio::sync::mpsc;

/// Parsed SSE event from the Anthropic API stream
pub enum SseEvent {
    Event(StreamEvent),
    Done,
}

/// Parse an SSE stream from an HTTP response into a channel of events.
pub async fn parse_sse_stream(
    response: Response,
    tx: mpsc::UnboundedSender<Result<SseEvent>>,
) {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(pos) = buffer.find("\n\n") {
                    let message = buffer[..pos].to_string();
                    buffer = buffer[pos + 2..].to_string();

                    if let Some(event) = parse_sse_message(&message) {
                        match event {
                            Ok(sse_event) => {
                                if tx.send(Ok(sse_event)).is_err() {
                                    return;
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e));
                                return;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(Err(anyhow!("Stream error: {}", e)));
                return;
            }
        }
    }

    let _ = tx.send(Ok(SseEvent::Done));
}

fn parse_sse_message(message: &str) -> Option<Result<SseEvent>> {
    let mut data_line = None;

    for line in message.lines() {
        if let Some(value) = line.strip_prefix("data: ") {
            data_line = Some(value);
        }
    }

    let data = data_line?;

    if data == "[DONE]" {
        return Some(Ok(SseEvent::Done));
    }

    match serde_json::from_str::<StreamEvent>(data) {
        Ok(event) => Some(Ok(SseEvent::Event(event))),
        Err(e) => Some(Err(anyhow!("Failed to parse SSE data: {} — raw: {}", e, data))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::StreamEvent;

    #[test]
    fn test_parse_text_delta() {
        let message = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}";
        let result = parse_sse_message(message).unwrap().unwrap();
        match result {
            SseEvent::Event(StreamEvent::ContentBlockDelta { index, .. }) => {
                assert_eq!(index, 0);
            }
            _ => panic!("Expected ContentBlockDelta event"),
        }
    }

    #[test]
    fn test_parse_message_stop() {
        let message = "event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let result = parse_sse_message(message).unwrap().unwrap();
        assert!(matches!(result, SseEvent::Event(StreamEvent::MessageStop)));
    }

    #[test]
    fn test_parse_done_signal() {
        let message = "data: [DONE]";
        let result = parse_sse_message(message).unwrap().unwrap();
        assert!(matches!(result, SseEvent::Done));
    }

    #[test]
    fn test_skip_comment_lines() {
        let result = parse_sse_message(": ping");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ping() {
        let message = "event: ping\ndata: {\"type\":\"ping\"}";
        let result = parse_sse_message(message).unwrap().unwrap();
        assert!(matches!(result, SseEvent::Event(StreamEvent::Ping)));
    }
}
