use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, error::Error, fmt};

const OPENAI_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_MODEL: &str = "gpt-5.1-mini";

#[derive(Clone, Debug, Default)]
pub struct GptConfig {
    api_key: Option<String>,
}

impl GptConfig {
    pub fn from_env() -> Self {
        let api_key = env::var("OPENAI_API_KEY").ok();
        Self { api_key }
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

#[derive(Clone)]
pub struct GptClient {
    http: Client,
    config: GptConfig,
}

impl GptClient {
    pub fn new(config: GptConfig) -> Self {
        let http = Client::builder()
            .user_agent("alaya-scan-cli/0.1.0")
            .build()
            .expect("failed to build reqwest client");

        Self { http, config }
    }

    pub fn has_api_key(&self) -> bool {
        self.config.api_key().is_some()
    }

    pub async fn summarize_book(&self, title: &str) -> Result<String, GptError> {
        let prompt = format!(
            "Give me a single concise sentence summarizing the book titled \"{title}\". \
            If you do not know it, reply with \"Summary unavailable.\""
        );

        let request = ChatCompletionRequest {
            model: DEFAULT_MODEL.to_string(),
            messages: vec![
                ChatMessage::system("You are a helpful literary assistant."),
                ChatMessage::user(prompt),
            ],
            temperature: Some(0.2),
        };

        let response = self.send_chat(request).await?;
        response
            .choices
            .into_iter()
            .map(|choice| choice.message.content)
            .find(|content| !content.trim().is_empty())
            .ok_or_else(|| GptError::UnexpectedResponse("Empty response from GPT-5".into()))
    }

    pub async fn send_chat(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, GptError> {
        let api_key = self
            .config
            .api_key()
            .ok_or(GptError::MissingApiKey)?
            .to_string();

        let response = self
            .http
            .post(OPENAI_CHAT_COMPLETIONS_URL)
            .bearer_auth(api_key)
            .json(&request)
            .send()
            .await
            .map_err(GptError::Http)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GptError::UnexpectedResponse(format!(
                "OpenAI request failed ({status}): {body}"
            )));
        }

        let payload = response.bytes().await.map_err(GptError::Http)?;
        serde_json::from_slice(&payload).map_err(GptError::Json)
    }
}

#[derive(Debug)]
pub enum GptError {
    MissingApiKey,
    Http(reqwest::Error),
    Json(serde_json::Error),
    UnexpectedResponse(String),
}

impl fmt::Display for GptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GptError::MissingApiKey => write!(f, "OPENAI_API_KEY is not set"),
            GptError::Http(err) => write!(f, "HTTP error: {err}"),
            GptError::Json(err) => write!(f, "Failed to parse response JSON: {err}"),
            GptError::UnexpectedResponse(msg) => write!(f, "{msg}"),
        }
    }
}

impl Error for GptError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            GptError::Http(err) => Some(err),
            GptError::Json(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system<T: Into<String>>(content: T) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user<T: Into<String>>(content: T) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessage,
}
