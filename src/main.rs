mod api;
mod auth;
mod cost;
mod repl;
mod session;
mod tools;
mod ui;

use clap::Parser;

#[derive(Parser)]
#[command(name = "mini-claude-code", about = "A minimal Claude Code CLI")]
struct Cli {
    /// Model to use
    #[arg(short, long, default_value = "claude-haiku-4-5-20251001")]
    model: String,

    /// Max tokens for response
    #[arg(long, default_value_t = 8192)]
    max_tokens: u32,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Get auth BEFORE starting tokio runtime (blocking reqwest can't run inside async)
    let auth = match auth::get_auth() {
        Ok(auth) => auth,
        Err(e) => {
            eprintln!("Authentication failed: {}", e);
            std::process::exit(1);
        }
    };

    // Now start the async runtime
    tokio::runtime::Runtime::new()?.block_on(async {
        let cwd = std::env::current_dir()?.display().to_string();

        let mut client = api::client::AnthropicClient::new(auth, cli.model, cli.max_tokens);
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
    })
}
