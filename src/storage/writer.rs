//! Async JSONL writer actor using mpsc channel pattern.
//! All file I/O is handled by a single background task.

use std::collections::HashMap;
use std::path::PathBuf;

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::types::StorageRecord;

/// Handle to the storage writer. Clone the sender to write from multiple tasks.
pub struct StorageWriter {
    tx: mpsc::Sender<StorageRecord>,
}

impl StorageWriter {
    /// Spawn the writer actor. Returns a handle with a cloneable sender.
    pub fn spawn(data_dir: PathBuf, token: CancellationToken) -> Self {
        let (tx, rx) = mpsc::channel::<StorageRecord>(4096);
        tokio::spawn(writer_task(data_dir, rx, token));
        Self { tx }
    }

    pub fn sender(&self) -> mpsc::Sender<StorageRecord> {
        self.tx.clone()
    }
}

/// Key for the open file handle map.
#[derive(Hash, Eq, PartialEq)]
struct FileKey {
    epoch: u64,
    source: String,
}

struct OpenFile {
    writer: tokio::io::BufWriter<tokio::fs::File>,
    lines_since_flush: usize,
}

async fn writer_task(
    data_dir: PathBuf,
    mut rx: mpsc::Receiver<StorageRecord>,
    token: CancellationToken,
) {
    let mut files: HashMap<FileKey, OpenFile> = HashMap::new();
    let mut flush_interval = tokio::time::interval(std::time::Duration::from_secs(5));
    flush_interval.tick().await; // consume first tick

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                info!("StorageWriter: cancellation received, draining...");
                // Drain remaining messages
                while let Ok(record) = rx.try_recv() {
                    write_record(&data_dir, &mut files, record).await;
                }
                flush_all(&mut files).await;
                info!("StorageWriter: shutdown complete");
                return;
            }

            _ = flush_interval.tick() => {
                flush_all(&mut files).await;
                // Close files for epochs that haven't been written to recently
                // (keeps file handles bounded)
            }

            msg = rx.recv() => {
                match msg {
                    Some(record) => {
                        write_record(&data_dir, &mut files, record).await;
                    }
                    None => {
                        info!("StorageWriter: channel closed, flushing...");
                        flush_all(&mut files).await;
                        return;
                    }
                }
            }
        }
    }
}

async fn write_record(
    data_dir: &PathBuf,
    files: &mut HashMap<FileKey, OpenFile>,
    record: StorageRecord,
) {
    let epoch = record.epoch.unwrap_or(0);
    let key = FileKey {
        epoch,
        source: record.source.clone(),
    };

    if !files.contains_key(&key) {
        match open_file(data_dir, epoch, &record.source).await {
            Ok(writer) => {
                files.insert(key, OpenFile { writer, lines_since_flush: 0 });
            }
            Err(e) => {
                error!(error = %e, epoch = epoch, source = %record.source, "Failed to open JSONL file");
                return;
            }
        }
    }

    let key = FileKey {
        epoch,
        source: record.source.clone(),
    };

    if let Some(open) = files.get_mut(&key) {
        match serde_json::to_string(&record) {
            Ok(line) => {
                if let Err(e) = open.writer.write_all(line.as_bytes()).await {
                    warn!(error = %e, "Failed to write record");
                    return;
                }
                if let Err(e) = open.writer.write_all(b"\n").await {
                    warn!(error = %e, "Failed to write newline");
                    return;
                }
                open.lines_since_flush += 1;

                if open.lines_since_flush >= 100 {
                    let _ = open.writer.flush().await;
                    open.lines_since_flush = 0;
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to serialize record");
            }
        }
    }
}

async fn open_file(
    data_dir: &PathBuf,
    epoch: u64,
    source: &str,
) -> anyhow::Result<tokio::io::BufWriter<tokio::fs::File>> {
    let dir = data_dir.join(epoch.to_string());
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{source}.jsonl"));
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    Ok(tokio::io::BufWriter::new(file))
}

async fn flush_all(files: &mut HashMap<FileKey, OpenFile>) {
    for (_, open) in files.iter_mut() {
        if open.lines_since_flush > 0 {
            let _ = open.writer.flush().await;
            open.lines_since_flush = 0;
        }
    }
}
