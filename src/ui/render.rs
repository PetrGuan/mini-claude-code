use std::io::{self, Write};

/// Print a chunk of streaming text directly to stdout.
/// Used during streaming to print text as it arrives.
pub fn print_stream_chunk(text: &str) {
    print!("{}", text);
    io::stdout().flush().ok();
}
