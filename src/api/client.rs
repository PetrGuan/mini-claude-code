use crate::api::stream::{parse_sse_stream, SseEvent};
use crate::api::types::{CreateMessageRequest, Message, ToolDefinition};
use crate::auth::AuthResult;
use anyhow::{anyhow, Result};
use reqwest::Client;
use tokio::sync::mpsc;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    client: Client,
    auth: AuthResult,
    pub model: String,
    pub max_tokens: u32,
    pub system_prompt: Option<String>,
}

impl AnthropicClient {
    pub fn new(auth: AuthResult, model: String, max_tokens: u32) -> Self {
        Self {
            client: Client::new(),
            auth,
            model,
            max_tokens,
            system_prompt: None,
        }
    }

    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
    }

    /// Send a streaming message request. Returns a channel that yields SSE events.
    pub async fn send_message_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<mpsc::UnboundedReceiver<Result<SseEvent>>> {
        let request = CreateMessageRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system: self.system_prompt.clone(),
            messages: messages.to_vec(),
            tools: tools.to_vec(),
            stream: true,
        };

        let mut req_builder = self
            .client
            .post(API_URL)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json");

        // Apply auth headers (API key or OAuth token)
        for (key, value) in self.auth.auth_headers() {
            req_builder = req_builder.header(&key, &value);
        }

        let response = req_builder.json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error ({}): {}", status, body));
        }

        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(parse_sse_stream(response, tx));

        Ok(rx)
    }
}
