use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, error::Error, fmt};

const OPENAI_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_MODEL: &str = "gpt-5-nano";
const USER_AGENT: &str = "alayascan/0.1.0";

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
            .user_agent(USER_AGENT)
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
        };

        let response = self.send_chat(request).await?;
        response
            .choices
            .into_iter()
            .map(|choice| choice.message.content)
            .find(|content| !content.trim().is_empty())
            .ok_or_else(|| GptError::UnexpectedResponse("Empty response from GPT-5-mini".into()))
    }

    pub async fn extract_book_metadata(
        &self,
        query: &str,
        model: &str,
    ) -> Result<BookMetadata, GptError> {
        let prompt = format!(
            "Identify this book: \"{query}\"\n\n\
            Return the information as JSON with these fields:\n\
            - title: the correct full title\n\
            - author: the correct author name\n\
            - isbn: the ISBN-13 if known, otherwise null\n\
            - publication_year: the original publication year if known, otherwise null\n\n\
            Return ONLY valid JSON, no other text."
        );

        let request = ChatCompletionRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage::system(
                    "You are a knowledgeable librarian assistant. \
                    Always respond with valid JSON only, no markdown or extra text.",
                ),
                ChatMessage::user(prompt),
            ],
        };

        let response = self.send_chat(request).await?;
        let content = response
            .choices
            .into_iter()
            .map(|choice| choice.message.content)
            .find(|content| !content.trim().is_empty())
            .ok_or_else(|| GptError::UnexpectedResponse("Empty response from GPT".into()))?;

        // Parse JSON response, stripping any markdown code fences if present
        let json_str = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        serde_json::from_str(json_str).map_err(|e| {
            GptError::UnexpectedResponse(format!(
                "Failed to parse book metadata: {e}\nRaw: {content}"
            ))
        })
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

        // Log the request
        println!("OpenAI API Request:");
        println!("  URL: {}", OPENAI_CHAT_COMPLETIONS_URL);
        println!("  Model: {}", request.model);
        for msg in &request.messages {
            println!(
                "  [{role}]: {content}",
                role = msg.role,
                content = msg.content
            );
        }

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

#[derive(Debug, Deserialize)]
pub struct BookMetadata {
    pub title: String,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub publication_year: Option<i32>,
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
    pub role: String,
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
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessage,
}
