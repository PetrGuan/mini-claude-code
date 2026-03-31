use clap::Parser;

#[derive(Parser)]
#[command(name = "mini-claude-code", about = "A minimal Claude Code CLI")]
struct Cli {
    /// Model to use
    #[arg(short, long, default_value = "claude-sonnet-4-20250514")]
    model: String,

    /// Max tokens for response
    #[arg(long, default_value_t = 8192)]
    max_tokens: u32,

    /// Initial prompt (if omitted, starts interactive REPL)
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set");

    println!("mini-claude-code v0.1.0");
    println!("Model: {}", cli.model);
    println!("(REPL not yet implemented)");

    Ok(())
}
