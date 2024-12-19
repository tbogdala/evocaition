#![allow(dead_code)]
use anyhow::{anyhow, Result};
use base64::{prelude::BASE64_STANDARD, Engine};
use core::str;
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;
use std::io::{self, Write};

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

pub struct ApiClient {
    // The configuration for the API client
    config: Config,
}

/// `ApiClient` is a struct responsible for interacting with an OpenAI compatible text generation API.
///
/// It handles both chat and plain text completion requests based on the configuration provided.
/// The client reads the prompt from either the configuration or standard input, constructs the appropriate
/// request body, and sends it to the OpenRouter AI API. It then processes the response, handling both
/// streaming and non-streaming responses, and outputs the results to standard output.
impl ApiClient {
    pub fn new(config: Config) -> Self {
        ApiClient { config }
    }

    /// Sends a completion request to the OpenRouter AI API based on the configuration provided.
    ///
    /// This method handles both chat and plain text completion requests. It reads the prompt from either
    /// the configuration or standard input, constructs the appropriate request body, and sends it to the
    /// text generation API. The method processes the response, handling both streaming and non-streaming
    /// responses, and outputs the results to standard output.
    ///
    /// # Steps:
    /// 1. **Read Prompt**: If a prompt is provided in the configuration, it uses that. Otherwise, it reads
    ///    the prompt from standard input.
    /// 2. **Construct Request**: Depending on whether the request is for plain text or chat completion,
    ///    it constructs the JSON request body accordingly.
    /// 3. **Send Request**: Sends the request to the API using the `reqwest` client.
    /// 4. **Handle Response**: Parses the response and prints the generated content.
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

        let client = Client::new();

        // determine if we're using the chat-compltion endpoint or not
        let url = if self.config.plain {
            format!("{}/v1/completions", self.config.api)
        } else {
            format!("{}/v1/chat/completions", self.config.api)
        };

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
                            let image_data = match std::fs::read(image_path) {
                                Ok(data) => data,
                                Err(e) => return Err(anyhow!("Failed to read image file: {}", e)),
                            };
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

        // post the request out to the API endpoint
        let mut response = client
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

        if self.config.stream {
            //let mut stream = response.bytes_stream();
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
                                            print!("{}", c.text);
                                        }
                                        Choice::Streaming(c) => {
                                            if let Some(content) = c.delta.content {
                                                print!("{}", content);
                                            }
                                        }
                                        Choice::NonStreaming(c) => {
                                            if let Some(content) = c.message.content {
                                                print!("{}", content);
                                            }
                                        }
                                    }
                                }
                                std::io::stdout().flush()?;
                            }
                            Err(_) => {
                                match serde_json::from_str::<ErrorResponseContainer>(json_str) {
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
                                }
                            }
                        }
                    }

                    // if the line didn't start with 'Data: ' then we just throw it away
                    // and trim it out of the buffer...

                    buffer = buffer[pos + 1..].to_string();
                    buffer = buffer.trim_start().to_string();
                }
            }
        } else {
            let response_text = response.text().await?;
            match serde_json::from_str::<Response>(&response_text) {
                Ok(api_result) => {
                    if let Some(choice) = api_result.choices.first() {
                        match choice {
                            Choice::NonChat(ncc) => println!("{}", ncc.text),
                            Choice::NonStreaming(nsc) => {
                                println!("{}", nsc.message.content.clone().unwrap_or_default())
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
        }

        Ok(())
    }
}
