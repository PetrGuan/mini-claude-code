use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const FRAME_MS: u64 = 80;

/// A terminal spinner that runs in a background thread.
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    /// Start a spinner with the given message (e.g., "Thinking...")
    pub fn start(message: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let msg = message.to_string();

        let handle = std::thread::spawn(move || {
            let mut i = 0;
            while running_clone.load(Ordering::Relaxed) {
                let frame = FRAMES[i % FRAMES.len()];
                print!("\r\x1b[2K  \x1b[36m{}\x1b[0m \x1b[2m{}\x1b[0m", frame, msg);
                io::stdout().flush().ok();
                std::thread::sleep(std::time::Duration::from_millis(FRAME_MS));
                i += 1;
            }
            // Clear the spinner line
            print!("\r\x1b[2K");
            io::stdout().flush().ok();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stop the spinner and clear its line
    #[allow(dead_code)]
    pub fn stop(self) {
        // drop will handle it
        drop(self);
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }
    }
}
