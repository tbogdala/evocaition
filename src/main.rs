mod api;
mod config;

use std::{io::Write, process::exit};

use api::ApiClient;
use config::Config;

#[tokio::main]
async fn main() {
    // parse all of our command line arguments
    let config = Config::from_cli();

    // create the API text generator object and pass it a function that, when
    // it gets a response from the AI, will just print out what it receives.
    let api_client = ApiClient::new(config, |s: &str| {
        print!("{}", s);
        let _ = std::io::stdout().flush();
    });

    // run the actual API call...
    if let Err(e) = api_client.do_completion().await {
        eprintln!("ERROR: {}", e);
        exit(1);
    }
}
