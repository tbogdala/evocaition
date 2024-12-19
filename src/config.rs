use clap::Parser;
use std::env;

#[derive(Debug, Parser)]
#[clap(
    name = "evocaition",
    version = "0.1.0",
    author = "Timothy Bogdala",
    about = "A command-line tool to interact with AI LLMs via APIs. Reads from STDIN if '--prompt' is not supplied."
)]
pub struct Config {
    #[clap(
        long("api"),
        value_name = "URL",
        help = "The API endpoint base URL to use.",
        default_value = "https://openrouter.ai/api"
    )]
    pub api: String,

    #[clap(
        long("key"),
        value_name = "API_KEY",
        help = "Sets the API key for remote endpoint; if absent, the envvar 'OPENROUTER_API_KEY' is checked",
        default_value = ""
    )]
    pub api_key: String,

    #[clap(
        long,
        value_name = "PROMPT",
        help = "Sets the prompt for the AI instead of reading from STDIN"
    )]
    pub prompt: Option<String>,

    #[clap(
        short('n'),
        long,
        value_name = "INT",
        help = "Sets the maximum number of tokens to generate in the completion"
    )]
    pub max_tokens: Option<u32>,

    #[clap(
        long,
        value_name = "MODEL_ID",
        help = "Sets the model to use for generating completions with the API",
        default_value = "google/gemini-2.0-flash-exp:free"
    )]
    pub model_id: String,

    #[clap(
        short('s'),
        long,
        value_name = "BOOL",
        help = "Write the response to stdout as it's received",
        default_value_t = false
    )]
    pub stream: bool,

    #[clap(
        long,
        value_name = "BOOL",
        help = "Set to use the non-chat completion API",
        default_value_t = false
    )]
    pub plain: bool,

    #[clap(long, value_name = "F32", help = "Sets the temperature for sampling")]
    pub temp: Option<f32>,

    #[clap(
        long,
        value_name = "F32",
        help = "Include only the top tokens whose probabilities add up to P when sampling"
    )]
    pub top_p: Option<f32>,

    #[clap(
        long,
        value_name = "F32",
        help = "The minimum probability for a token relative to the most probable token when sampling"
    )]
    pub min_p: Option<f32>,

    #[clap(
        long,
        value_name = "INT",
        help = "Include only this amount of top tokens when sampling"
    )]
    pub top_k: Option<u32>,

    #[clap(
        long,
        value_name = "F32",
        help = "A higher value makes the model less likely to repeat tokens"
    )]
    pub rep_pen: Option<f32>,

    #[clap(
        long,
        value_name = "INT",
        help = "The seed to use for the generation (determinism is not guaranteed)"
    )]
    pub seed: Option<i64>,

    #[clap(
        long("image"),
        value_name = "FILEPATH or URL",
        help = "An image to attach to the user's request; '--plain' must not be used."
    )]
    pub image_file: Option<String>,
}

impl Config {
    pub fn from_cli() -> Self {
        let mut config = Config::parse();

        // Fallback to environment variable if api_key is not provided
        if config.api_key.is_empty() {
            match env::var("OPENROUTER_API_KEY") {
                Ok(key) => {
                    config.api_key = key;
                    config
                }
                Err(_) => panic!(
                    "API key must be provided via --key or OPENROUTER_API_KEY environment variable"
                ),
            }
        } else {
            config
        }
    }
}
