use rusqlite::Connection;
use std::sync::mpsc;
use std::thread;

/// A structured log record sent to the background writer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogRecord {
    pub id: String,
    pub service: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields_json: String,
    pub timestamp: String,
    pub error_code: Option<String>,
}

/// A clonable sender handle that the DatastoreLayer uses.
#[derive(Clone)]
pub struct LogSender {
    tx: mpsc::Sender<WriterMsg>,
}

impl LogSender {
    pub fn send(&self, record: LogRecord) {
        let _ = self.tx.send(WriterMsg::Log(record));
    }
}

enum WriterMsg {
    Log(LogRecord),
    Shutdown,
}

/// Background SQLite writer that receives log records via a channel.
pub struct LogWriter {
    tx: mpsc::Sender<WriterMsg>,
    handle: Option<thread::JoinHandle<()>>,
}

impl LogWriter {
    /// Open (or create) the log database and start the background writer thread.
    pub fn open(db_path: &str, _service: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;

        // System logs table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _system_logs (
                id TEXT PRIMARY KEY,
                service TEXT NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                message TEXT NOT NULL,
                fields_json TEXT DEFAULT '{}',
                error_code TEXT,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_system_logs_service_ts
             ON _system_logs (service, timestamp)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_system_logs_level
             ON _system_logs (level, timestamp)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_system_logs_error_code
             ON _system_logs (error_code) WHERE error_code IS NOT NULL",
            [],
        )?;

        // Error reports table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _error_reports (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                code TEXT NOT NULL,
                message TEXT NOT NULL,
                detail TEXT,
                severity TEXT NOT NULL,
                retryable INTEGER NOT NULL DEFAULT 0,
                suggestion TEXT NOT NULL,
                service TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_error_reports_code
             ON _error_reports (code, timestamp)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_error_reports_service
             ON _error_reports (service, timestamp)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_error_reports_severity
             ON _error_reports (severity, timestamp)",
            [],
        )?;

        let (tx, rx) = mpsc::channel::<WriterMsg>();

        let handle = thread::spawn(move || {
            Self::writer_loop(conn, rx);
        });

        Ok(Self {
            tx,
            handle: Some(handle),
        })
    }

    /// Get a clonable sender for the DatastoreLayer to use.
    pub fn sender(&self) -> LogSender {
        LogSender {
            tx: self.tx.clone(),
        }
    }

    fn writer_loop(conn: Connection, rx: mpsc::Receiver<WriterMsg>) {
        let mut batch: Vec<LogRecord> = Vec::with_capacity(64);
        let flush_interval = std::time::Duration::from_secs(1);

        loop {
            match rx.recv_timeout(flush_interval) {
                Ok(WriterMsg::Log(record)) => {
                    batch.push(record);
                    while batch.len() < 256 {
                        match rx.try_recv() {
                            Ok(WriterMsg::Log(r)) => batch.push(r),
                            Ok(WriterMsg::Shutdown) => {
                                Self::flush_batch(&conn, &batch);
                                return;
                            }
                            Err(_) => break,
                        }
                    }
                }
                Ok(WriterMsg::Shutdown) => {
                    Self::flush_batch(&conn, &batch);
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    Self::flush_batch(&conn, &batch);
                    return;
                }
            }

            if !batch.is_empty() {
                Self::flush_batch(&conn, &batch);
                batch.clear();
            }
        }
    }

    fn flush_batch(conn: &Connection, batch: &[LogRecord]) {
        if batch.is_empty() {
            return;
        }
        if let Err(e) = conn.execute_batch("BEGIN") {
            eprintln!("[panorama-logging] begin failed: {e}");
            return;
        }
        for record in batch {
            if let Err(e) = conn.execute(
                "INSERT OR IGNORE INTO _system_logs
                    (id, service, level, target, message, fields_json, error_code, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    record.id,
                    record.service,
                    record.level,
                    record.target,
                    record.message,
                    record.fields_json,
                    record.error_code,
                    record.timestamp,
                ],
            ) {
                eprintln!("[panorama-logging] insert failed: {e}");
            }
        }
        if let Err(e) = conn.execute_batch("COMMIT") {
            eprintln!("[panorama-logging] commit failed: {e}");
        }
    }
}

impl Drop for LogWriter {
    fn drop(&mut self) {
        let _ = self.tx.send(WriterMsg::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Persist a PanoramaError to the _error_reports table.
/// Called from panorama-errors IntoResponse or explicitly.
pub fn persist_error(sender: &LogSender, error: &panorama_errors::PanoramaError) {
    // Send the error as a log record with the error_code set
    sender.send(LogRecord {
        id: uuid::Uuid::new_v4().to_string(),
        service: error.service.clone(),
        level: "ERROR".to_string(),
        target: format!("panorama_errors::{}", error.code),
        message: format!("[{}] {}", error.code, error.message),
        fields_json: serde_json::to_string(error).unwrap_or_default(),
        timestamp: error.timestamp.clone(),
        error_code: Some(error.code.clone()),
    });
}
