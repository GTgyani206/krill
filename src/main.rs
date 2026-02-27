mod client;
mod error;
mod types;

use clap::{Parser, Subcommand};
use colored::Colorize;
use futures_util::StreamExt;
use std::process;

use crate::client::TinyFishClient;
use crate::types::{AutomationRequest, SseEvent};

#[derive(Parser)]
#[command(version, about = "CLI for the TinyFish web automation API")]
struct Cli {
    /// API key for TinyFish
    #[arg(short, long, env = "TINYFISH_API_KEY")]
    api_key: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an automation task
    Run {
        /// The URL to start the automation on
        url: String,

        /// The goal of the automation task
        goal: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { url, goal } => {
            if let Err(e) = run_automation(cli.api_key, url, goal).await {
                eprintln!("{}", format!("Error: {}", e).red());
                process::exit(1);
            }
        }
    }
}

async fn run_automation(api_key: String, url: String, goal: String) -> Result<(), crate::error::KrillError> {
    let client = TinyFishClient::new(api_key);

    let payload = AutomationRequest {
        url,
        goal,
        browser_profile: None,
        proxy_config: None,
        api_integration: None,
        feature_flags: None,
    };

    let stream = client.run_sse(payload).await?;
    tokio::pin!(stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(event) => {
                match event {
                    SseEvent::Progress { purpose, message, .. } => {
                        let p = purpose.unwrap_or_default();
                        let m = message.unwrap_or_default();
                        
                        let log_line = if p.is_empty() && m.is_empty() {
                            String::new()
                        } else if p.is_empty() {
                            m
                        } else if m.is_empty() {
                            p
                        } else {
                            format!("[{}] {}", p, m)
                        };
                        
                        if !log_line.is_empty() {
                            eprintln!("{}", log_line.dimmed());
                        }
                    }
                    SseEvent::Complete { result_json, .. } => {
                        if let Some(json) = result_json {
                            match serde_json::to_string_pretty(&json) {
                                Ok(pretty) => println!("{}", pretty),
                                Err(e) => {
                                    eprintln!("{}", format!("Error formatting JSON: {}", e).red());
                                    process::exit(1);
                                }
                            }
                        } else {
                            println!("{{}}");
                        }
                    }
                    _ => {
                        // Ignore Heartbeat, Started, StreamingUrl, etc.
                    }
                }
            }
            Err(e) => {
                eprintln!("{}", format!("Stream error: {}", e).red());
                process::exit(1);
            }
        }
    }

    Ok(())
}
