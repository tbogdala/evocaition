#![allow(dead_code)]
use anyhow::{anyhow, Result};
use base64::{prelude::BASE64_STANDARD, Engine};
use core::str;
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;
use std::io::{self};

use crate::config::Config;

#[derive(Debug, Deserialize, Clone)]
struct Response {
    // Note: docs don't specify this as optional, but it was noticed in practice
    id: Option<String>,
    provider: Option<String>,
    model: String,

    // "chat.completion" or "chat.completion.chunk"
    object: String,

    // Unix timestamp
    created: u64,

    // Depending on whether you set "stream" to "true" and
    // whether you passed in "messages" or a "prompt", you
    // will get a different output shape
    choices: Vec<Choice>,

    // Only present if the provider supports it
    system_fingerprint: Option<String>,

    // Usage data is always returned for non-streaming.
    // When streaming, you will get one usage object at
    // the end accompanied by an empty choices array.
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum Choice {
    NonChat(NonChatChoice),
    NonStreaming(NonStreamingChoice),
    Streaming(StreamingChoice),
}

#[derive(Debug, Deserialize, Clone)]
struct NonChatChoice {
    finish_reason: Option<String>,
    text: String,
    error: Option<ErrorResponse>,
}

#[derive(Debug, Deserialize, Clone)]
struct NonStreamingChoice {
    // Depends on the model. Ex: 'stop' | 'length' | 'content_filter' | 'tool_calls'
    finish_reason: Option<String>,
    message: Message,
    error: Option<ErrorResponse>,
}

#[derive(Debug, Deserialize, Clone)]
struct StreamingChoice {
    finish_reason: Option<String>,
    delta: Delta,
    error: Option<ErrorResponse>,
}

#[derive(Debug, Deserialize, Clone)]
struct Message {
    content: Option<String>,
    role: String,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize, Clone)]
struct Delta {
    content: Option<String>,
    role: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize, Clone)]
struct ErrorResponse {
    code: i32,
    message: String,

    // Contains additional error information such as provider details, the raw error message, etc.
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct ErrorResponseContainer {
    error: ErrorResponse,
}

#[derive(Debug, Deserialize, Clone)]
struct ToolCall {
    id: String,
    r#type: String,
    function: FunctionCall,
}

#[derive(Debug, Deserialize, Clone)]
struct FunctionCall {
    // Define the fields of FunctionCall based on your needs
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
struct Usage {
    // Including images and tools if any
    prompt_tokens: u64,

    // The tokens generated
    completion_tokens: u64,

    // Sum of the above two fields
    total_tokens: u64,
}

pub type ApiClientCallback = fn(&str);

pub struct ApiClient {
    // The configuration for the API client
    config: Config,

    // The callback that will get either the entire response when received,
    // or a streaming update, piece by piece, if streaming is enabled in `config`.
    callback: ApiClientCallback,
}

/// `ApiClient` is a struct responsible for interacting with an OpenAI compatible text generation API.
///
/// It handles both chat and plain text completion requests based on the configuration provided.
/// The client reads the prompt from either the configuration or standard input, constructs the appropriate
/// request body, and sends it to the OpenRouter AI API. It then processes the response, handling both
/// streaming and non-streaming responses, and outputs the results to the callback function provided.
impl ApiClient {
    pub fn new(config: Config, callback: ApiClientCallback) -> Self {
        ApiClient { config, callback }
    }

    /// Sends a completion request to the OpenRouter AI API based on the configuration provided.
    ///
    /// This method handles both chat and plain text completion requests. It reads the prompt from either
    /// the configuration or standard input, constructs the appropriate request body, and sends it to the
    /// text generation API. The method processes the response, handling both streaming and non-streaming
    /// responses, and outputs the results to callback function passed in when creating the `ApiClient` object.
    ///
    /// # Returns:
    /// - `Result<()>`: Returns Ok() if the completion request is successful and the response is
    ///   processed without errors or an Err if there is a failure in reading the
    ///   prompt, sending the request, or processing the response.
    pub async fn do_completion(&self) -> Result<()> {
        // Read the prompt from stdin if the prompt wasn't supplied
        let prompt = match &self.config.prompt {
            Some(p) => p.clone(),
            None => io::read_to_string(io::stdin())?,
        };

        // determine if we're using the chat-compltion endpoint or not
        let url = if self.config.plain {
            format!("{}/v1/completions", self.config.api)
        } else {
            format!("{}/v1/chat/completions", self.config.api)
        };

        // build the response body for the request using the prompt and all of
        // the configuration settings for this ApiClient.
        let body = self.build_request_body(&prompt);

        // post the request out to the API endpoint
        let client = Client::new();
        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("HTTP-Referer", "https://github.com/tbogdala/evocaition")
            .header("X-Title", "evocaition")
            .json(&body)
            .send()
            .await?;
        if !response.status().is_success() {
            let error_message = format!(
                "API request failed with status {}: {}",
                response.status(),
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unable to read response body".to_string())
            );
            return Err(anyhow!(error_message));
        }

        // handle the response in one of two ways depending on whether or not 'streaming'
        // is configured.
        if self.config.stream {
            self.process_streaming_response(response).await?;
        } else {
            let response_text = response.text().await?;
            self.process_non_streaming_response(&response_text)?;
        }

        Ok(())
    }

    /// Constructs the request body for an API call based on the provided prompt and configuration.
    ///
    /// The function constructs a JSON request body based on the configuration specified in `config`.
    /// The structure of the request body differs slightly depending on whether plain mode is enabled or not:
    ///
    /// - **Plain Mode (self.config.plain = true):**
    ///   - Includes only the `model`, `prompt`, and `stream` fields.
    ///
    /// - **Chat Mode (self.config.plain = false):**
    ///   - Includes the `model`, `messages`, and `stream` fields.
    ///   - If an image file path is provided (`self.config.image_file`), the function includes the image
    ///     in the `messages` array. If the image provided is a URL, then just the URL is added to the prompt.
    ///     Otherwise it is assumed to be a filesystem path and the image is read from the file system,
    ///     converted to base64, and the MIME type is determined based on the file extension.
    ///   - If no image file is provided, only the user's prompt is included in the `messages` array.
    ///
    /// Additionally, the function optionally includes other fields (`max_tokens`, `temperature`, `top_k`,
    /// `top_p`, `min_p`, `repetition_penalty`, `seed`, etc...) in the request body if they are set in the configuration.
    ///
    /// # Parameters
    /// - `prompt` - A string slice representing the user's input prompt to be sent to the model.
    ///
    /// # Returns
    /// A `serde_json::Value` representing the JSON request body to be sent in the API call.
    fn build_request_body(&self, prompt: &str) -> serde_json::Value {
        let mut body = if self.config.plain {
            json!({
                "model": self.config.model_id,
                "prompt": prompt,
                "stream": self.config.stream,
            })
        } else {
            // Handle image inclusion if config.image_file is set
            let messages = if let Some(image_path) = &self.config.image_file {
                let image_content = match Url::parse(image_path) {
                    Ok(_url) => image_path.clone(),
                    Err(_) => {
                        // Determine the image type based on the file extension
                        let mime_type = match image_path.split('.').last().unwrap_or_default() {
                            "jpg" | "jpeg" => Some("image/jpeg"),
                            "png" => Some("image/png"),
                            "webp" => Some("image/webp"),
                            _ => None,
                        };

                        if let Some(mime_type) = mime_type {
                            // Read the image file
                            let image_data =
                                std::fs::read(image_path).expect("Failed to read image file");
                            // Encode image to base64
                            format!(
                                "data:{};base64,{}",
                                mime_type,
                                BASE64_STANDARD.encode(&image_data)
                            )
                        } else {
                            "".to_string()
                        }
                    }
                };

                vec![
                    json!({
                        "role": "user",
                        "content":[
                            {
                                "type": "image_url",
                                "image_url": {
                                    "url":  image_content,
                                },
                            },
                        ]
                    }),
                    json!({
                        "role": "user",
                        "content": prompt
                    }),
                ]
            } else {
                vec![json!({
                    "role": "user",
                    "content": prompt,
                })]
            };
            json!({
                "model": self.config.model_id,
                "messages": messages,
                "stream": self.config.stream,
            })
        };

        // add in some optional parameters to the request
        if let Some(max_tokens) = self.config.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temp) = self.config.temp {
            body["temperature"] = json!(temp);
        }
        if let Some(top_k) = self.config.top_k {
            body["top_k"] = json!(top_k);
        }
        if let Some(top_p) = self.config.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(min_p) = self.config.min_p {
            body["min_p"] = json!(min_p);
        }
        if let Some(rep_pen) = self.config.rep_pen {
            body["repetition_penalty"] = json!(rep_pen);
        }
        if let Some(seed) = self.config.seed {
            body["seed"] = json!(seed);
        }

        body
    }

    /// Processes a streaming HTTP response, handling JSON data chunks and invoking callbacks for each message.
    ///
    /// This function asynchronously reads chunks from a `reqwest::Response` object, decodes them from UTF-8,
    /// and processes lines that start with the prefix "data: ". Each valid JSON message is parsed into a `Response`
    /// object, and the appropriate callback is invoked based on the type of choice contained within the response.
    ///
    /// # Parameters
    /// - `response`: A mutable `reqwest::Response` object representing the incoming HTTP response which
    ///   should already have been sent.
    ///
    /// # Returns
    /// - Returns `Ok(())` if the response was processed successfully, or an `Err` if an error
    ///   occurred during processing.
    ///
    /// # Notes
    /// - The buffer is trimmed to remove leading and trailing whitespace after processing each line.
    async fn process_streaming_response(&self, mut response: reqwest::Response) -> Result<()> {
        let mut buffer = String::new();

        while let Ok(Some(chunk)) = response.chunk().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete lines from the buffer
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim();

                // Skip empty lines
                if line.is_empty() {
                    continue;
                }

                // Check if line starts with "data: " and parse the JSON
                if let Some(json_str) = line.strip_prefix("data: ") {
                    if json_str.trim() == "[DONE]" {
                        break;
                    }

                    match serde_json::from_str::<Response>(json_str) {
                        Ok(response) => {
                            for choice in response.choices {
                                match choice {
                                    Choice::NonChat(c) => {
                                        (self.callback)(&c.text);
                                    }
                                    Choice::Streaming(c) => {
                                        if let Some(content) = c.delta.content {
                                            (self.callback)(&content);
                                        }
                                    }
                                    Choice::NonStreaming(c) => {
                                        if let Some(content) = c.message.content {
                                            (self.callback)(&content);
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => match serde_json::from_str::<ErrorResponseContainer>(json_str) {
                            Ok(error_contaner) => {
                                return Err(anyhow::Error::msg(format!(
                                    "API request failed with code {}: {}\nError metadata:{:?}",
                                    error_contaner.error.code,
                                    error_contaner.error.message,
                                    error_contaner.error.metadata,
                                )));
                            }
                            Err(e) => {
                                return Err(anyhow!(
                                    "Failed to parse JSON: {}\nRaw JSON: {}",
                                    e,
                                    json_str
                                ));
                            }
                        },
                    }
                }

                // if the line didn't start with 'Data: ' then we just throw it away
                // and trim it out of the buffer...

                buffer = buffer[pos + 1..].to_string();
                buffer = buffer.trim_start().to_string();
            }
        }

        Ok(())
    }

    /// Processes a non-streaming JSON response from an API.
    ///
    /// This function takes a JSON-formatted string response, parses it to determine the type of response,
    /// and then either processes the response data or handles any errors.
    ///
    /// # Parameters
    /// - `response_text`: A string slice containing the JSON response text from the API.
    ///
    /// # Returns
    /// - An empty `Result` indicating success or an Err indicating failure.
    fn process_non_streaming_response(&self, response_text: &str) -> Result<()> {
        match serde_json::from_str::<Response>(&response_text) {
            Ok(api_result) => {
                if let Some(choice) = api_result.choices.first() {
                    match choice {
                        Choice::NonChat(ncc) => (self.callback)(&ncc.text),
                        Choice::NonStreaming(nsc) => {
                            (self.callback)(&nsc.message.content.clone().unwrap_or_default())
                        }
                        Choice::Streaming(_) => {
                            panic!("Shouldn't be getting streaming responses here...")
                        }
                    }
                }
            }
            Err(_) => match serde_json::from_str::<ErrorResponseContainer>(&response_text) {
                Ok(error_container) => {
                    return Err(anyhow::Error::msg(format!(
                        "API request failed with code {}: {}\nError metadata:{:?}",
                        error_container.error.code,
                        error_container.error.message,
                        error_container.error.metadata,
                    )));
                }
                Err(e) => {
                    return Err(anyhow!(
                        "Failed to parse JSON: {}\nRaw JSON: {}",
                        e,
                        response_text
                    ));
                }
            },
        }

        Ok(())
    }
}
