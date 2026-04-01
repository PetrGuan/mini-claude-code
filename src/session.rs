use crate::api::types::{ContentBlock, Message};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const BASE_DIR: &str = ".mini-claude-code";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum SessionLine {
    #[serde(rename = "meta")]
    Meta {
        session_id: String,
        created: String,
        cwd: String,
        model: String,
    },
    #[serde(rename = "message")]
    Message {
        #[serde(flatten)]
        message: Message,
    },
}

#[allow(dead_code)]
pub struct Session {
    pub id: String,
    pub model: String,
    path: PathBuf,
    file: File,
}

impl Session {
    pub fn new(cwd: &str, model: &str) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        let dir = project_dir(cwd)?;
        fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{}.jsonl", id));
        let mut file = File::create(&path)?;

        let now = humantime::format_rfc3339(SystemTime::now()).to_string();
        let meta = SessionLine::Meta {
            session_id: id.clone(),
            created: now,
            cwd: cwd.to_string(),
            model: model.to_string(),
        };
        writeln!(file, "{}", serde_json::to_string(&meta)?)?;
        file.flush()?;

        Ok(Self {
            id,
            model: model.to_string(),
            path,
            file,
        })
    }

    pub fn append_message(&mut self, message: &Message) -> Result<()> {
        let line = SessionLine::Message {
            message: message.clone(),
        };
        writeln!(self.file, "{}", serde_json::to_string(&line)?)?;
        self.file.flush()?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<(String, Vec<Message>)> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut model = String::new();
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionLine>(&line) {
                Ok(SessionLine::Meta { model: m, .. }) => {
                    model = m;
                }
                Ok(SessionLine::Message { message }) => {
                    messages.push(message);
                }
                Err(_) => continue,
            }
        }

        if model.is_empty() {
            return Err(anyhow!("Session file missing metadata"));
        }

        Ok((model, messages))
    }

    pub fn open_existing(path: &Path, model: &str) -> Result<Self> {
        let file = OpenOptions::new().append(true).open(path)?;
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            id,
            model: model.to_string(),
            path: path.to_path_buf(),
            file,
        })
    }
}

pub struct SessionInfo {
    pub path: PathBuf,
    pub title: String,
    pub modified: SystemTime,
    pub message_count: usize,
}

pub fn list_sessions(cwd: &str) -> Result<Vec<SessionInfo>> {
    let dir = match project_dir(cwd) {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let modified = entry.metadata()?.modified()?;

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut title = String::new();
        let mut message_count = 0;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            if let Ok(parsed) = serde_json::from_str::<SessionLine>(&line) {
                match parsed {
                    SessionLine::Message { message } => {
                        message_count += 1;
                        if title.is_empty() {
                            if let Some(text) = message.content.iter().find_map(|b| match b {
                                ContentBlock::Text { text } => Some(text.as_str()),
                                _ => None,
                            }) {
                                title = text.chars().take(50).collect();
                            }
                        }
                    }
                    SessionLine::Meta { .. } => {}
                }
            }
        }

        if title.is_empty() {
            title = "(empty session)".to_string();
        }

        sessions.push(SessionInfo {
            path,
            title,
            modified,
            message_count,
        });
    }

    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(sessions)
}

pub fn most_recent_session(cwd: &str) -> Result<Option<PathBuf>> {
    let sessions = list_sessions(cwd)?;
    Ok(sessions.into_iter().next().map(|s| s.path))
}

fn project_dir(cwd: &str) -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| anyhow!("HOME not set"))?;
    let sanitized = cwd.replace('/', "-");
    Ok(PathBuf::from(home)
        .join(BASE_DIR)
        .join("projects")
        .join(sanitized))
}

pub fn format_relative_time(time: SystemTime) -> String {
    let elapsed = SystemTime::now()
        .duration_since(time)
        .unwrap_or_default();
    let secs = elapsed.as_secs();

    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        let mins = secs / 60;
        format!("{} min{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if secs < 86400 {
        let hours = secs / 3600;
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else {
        let days = secs / 86400;
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::Role;
    use tempfile::TempDir;

    fn test_session_in(dir: &Path, model: &str) -> Session {
        let id = uuid::Uuid::new_v4().to_string();
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{}.jsonl", id));
        let mut file = File::create(&path).unwrap();
        let meta = SessionLine::Meta {
            session_id: id.clone(),
            created: "2026-03-31T12:00:00Z".to_string(),
            cwd: "/test".to_string(),
            model: model.to_string(),
        };
        writeln!(file, "{}", serde_json::to_string(&meta).unwrap()).unwrap();
        Session {
            id,
            model: model.to_string(),
            path,
            file,
        }
    }

    #[test]
    fn test_append_and_load() {
        let dir = TempDir::new().unwrap();
        let mut session = test_session_in(dir.path(), "haiku");

        let msg = Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "hello".to_string(),
            }],
        };
        session.append_message(&msg).unwrap();

        let (model, messages) = Session::load(&session.path).unwrap();
        assert_eq!(model, "haiku");
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_format_relative_time() {
        let now = SystemTime::now();
        assert_eq!(format_relative_time(now), "just now");

        let one_hour_ago = now - std::time::Duration::from_secs(3600);
        assert_eq!(format_relative_time(one_hour_ago), "1 hour ago");

        let two_days_ago = now - std::time::Duration::from_secs(86400 * 2);
        assert_eq!(format_relative_time(two_days_ago), "2 days ago");
    }
}
