mod api;
mod config;

use std::process::exit;

use api::ApiClient;
use config::Config;

#[tokio::main]
async fn main() {
    let config = Config::from_cli();
    let api_client = ApiClient::new(config);

    if let Err(e) = api_client.do_completion().await {
        eprintln!("ERROR: {}", e);
        exit(1);
    }
}
