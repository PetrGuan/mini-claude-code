use termimad::MadSkin;

pub fn create_skin() -> MadSkin {
    let mut skin = MadSkin::default();
    skin
}

pub fn render_markdown(text: &str, skin: &MadSkin) {
    skin.print_text(text);
}

pub fn print_stream_chunk(text: &str) {
    use std::io::{self, Write};
    print!("{}", text);
    io::stdout().flush().ok();
}
