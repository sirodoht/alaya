use alaya::gpt::{GptClient, GptConfig, GptError};
use std::{env, process};

#[tokio::main]
async fn main() {
    let title = parse_title();

    let config = GptConfig::from_env();
    if config.api_key().is_none() {
        eprintln!(
            "OPENAI_API_KEY is not configured. Please export it before running the alayascan command."
        );
        process::exit(1);
    }

    let client = GptClient::new(config);
    if let Err(error) = run_scan(&client, &title).await {
        eprintln!("Failed to summarize \"{title}\": {error}");
        process::exit(1);
    }
}

fn parse_title() -> String {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        eprintln!("Usage: cargo run --bin alayascan \"Book Title\"");
        process::exit(1);
    }

    args.join(" ")
}

async fn run_scan(client: &GptClient, title: &str) -> Result<(), GptError> {
    println!("Scanning \"{title}\"...");
    let summary = client.summarize_book(title).await?;
    println!("\nSummary: {summary}");
    Ok(())
}
