//! Small asynchronous structured logger for local diagnostics.
use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::mpsc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct Logger {
    sender: mpsc::SyncSender<String>,
}

impl Logger {
    pub fn start(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let (sender, receiver) = mpsc::sync_channel::<String>(2048);
        thread::Builder::new()
            .name("voxel-logger".into())
            .spawn(move || {
                let mut file = file;
                while let Ok(line) = receiver.recv() {
                    let _ = writeln!(file, "{line}");
                    let _ = file.flush();
                }
            })?;
        Ok(Self { sender })
    }

    pub fn event(&self, level: &str, name: &str, detail: impl AsRef<str>) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let line = format!(
            "{}.{} [{}] event={} {}",
            timestamp.as_secs(),
            timestamp.subsec_millis(),
            level,
            name,
            detail.as_ref()
        );
        let _ = self.sender.try_send(line);
    }
}
