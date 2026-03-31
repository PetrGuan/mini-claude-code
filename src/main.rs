mod api;
mod repl;
mod tools;
mod ui;

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set");

    let cwd = std::env::current_dir()?.display().to_string();

    let mut client = api::client::AnthropicClient::new(api_key, cli.model, cli.max_tokens);
    client.set_system_prompt(format!(
        "You are a helpful coding assistant running in the terminal.\n\
         Working directory: {}\n\
         You have access to tools for running bash commands, reading/writing/editing files, \
         and searching with glob patterns and grep.\n\
         When using tools, always use absolute paths.\n\
         Be concise in your responses.",
        cwd
    ));

    let registry = tools::create_default_registry();

    repl::run(&client, &registry).await
}
