//! Over-engineered structured logging: leveled `Logger`, pluggable
//! `LogSink`s, per-logger MDC-style context, and a tiny formatting layer.
//! (Yes, a light-link demo with a full logging framework — that's the
//! "hilarious" part.)

use std::io::Write;
use std::sync::{Arc, Mutex};

use crate::errors::{ErrorContext, PixferError};

/// Log levels, ordered by rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

impl LogLevel {
    pub fn rank(self) -> u8 {
        self as u8
    }
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }
    pub fn parse(s: &str) -> Option<LogLevel> {
        match s.to_uppercase().as_str() {
            "TRACE" => Some(LogLevel::Trace),
            "DEBUG" => Some(LogLevel::Debug),
            "INFO" => Some(LogLevel::Info),
            "WARN" => Some(LogLevel::Warn),
            "ERROR" => Some(LogLevel::Error),
            "FATAL" => Some(LogLevel::Fatal),
            _ => None,
        }
    }
}

/// A single structured log record.
#[derive(Debug, Clone)]
pub struct LogRecord {
    pub level: LogLevel,
    pub logger: String,
    pub message: String,
    pub context: Option<ErrorContext>,
    pub error: Option<PixferError>,
}

/// Sink trait so output can be redirected (stdout, in-memory, file).
pub trait LogSink: Send {
    fn emit(&mut self, rec: &LogRecord);
}

/// Writes formatted records to any `Write`.
pub struct WriterSink<W: Write> {
    writer: W,
}

impl<W: Write> WriterSink<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write + Send> LogSink for WriterSink<W> {
    fn emit(&mut self, rec: &LogRecord) {
        let _ = writeln!(
            self.writer,
            "[{}] [{}] {}{}",
            rec.level.as_str(),
            rec.logger,
            rec.message,
            rec.error
                .as_ref()
                .map(|e| format!(" | err={}", e))
                .unwrap_or_default()
        );
        let _ = self.writer.flush();
    }
}

/// In-memory sink for tests and metrics.
#[derive(Debug, Default, Clone)]
pub struct MemorySink {
    pub records: Vec<LogRecord>,
}

impl LogSink for MemorySink {
    fn emit(&mut self, rec: &LogRecord) {
        self.records.push(rec.clone());
    }
}

/// A sink that shares its recorded logs with outside observers through an
/// `Arc<Mutex>` so other threads (or the test itself) can read what the
/// logger emitted without taking ownership of the sink.
pub struct SharedSink {
    records: Arc<Mutex<Vec<LogRecord>>>,
}

impl SharedSink {
    /// Returns the sink plus a handle to the shared record list.
    pub fn new() -> (Self, Arc<Mutex<Vec<LogRecord>>>) {
        let records = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                records: records.clone(),
            },
            records,
        )
    }
}

impl LogSink for SharedSink {
    fn emit(&mut self, rec: &LogRecord) {
        self.records.lock().unwrap().push(rec.clone());
    }
}

/// The logger: a named, leveled, context-aware facade over a `Box<dyn LogSink>`.
pub struct Logger {
    name: String,
    level: LogLevel,
    sink: Box<dyn LogSink>,
}

impl Logger {
    pub fn new(name: &str, level: LogLevel, sink: Box<dyn LogSink>) -> Self {
        Self {
            name: name.to_string(),
            level,
            sink,
        }
    }

    pub fn with_context(mut self, ctx: ErrorContext) -> Self {
        // (Over-engineered: contextual logger.)
        self.name = format!("{} (ctx:{})", self.name, ctx.component);
        self
    }

    fn log(&mut self, level: LogLevel, msg: &str, err: Option<&PixferError>) {
        if level.rank() < self.level.rank() {
            return;
        }
        let rec = LogRecord {
            level,
            logger: self.name.clone(),
            message: msg.to_string(),
            context: None,
            error: err.map(|e| e.clone()),
        };
        self.sink.emit(&rec);
    }

    pub fn trace(&mut self, msg: &str) {
        self.log(LogLevel::Trace, msg, None);
    }
    pub fn debug(&mut self, msg: &str) {
        self.log(LogLevel::Debug, msg, None);
    }
    pub fn info(&mut self, msg: &str) {
        self.log(LogLevel::Info, msg, None);
    }
    pub fn warn(&mut self, msg: &str) {
        self.log(LogLevel::Warn, msg, None);
    }
    pub fn error(&mut self, msg: &str) {
        self.log(LogLevel::Error, msg, None);
    }
    pub fn fatal(&mut self, msg: &str) {
        self.log(LogLevel::Fatal, msg, None);
    }
    pub fn error_with(&mut self, msg: &str, err: &PixferError) {
        self.log(LogLevel::Error, msg, Some(err));
    }
}

/// Global-ish default logger using a `MemorySink` (swappable in tests).
#[derive(Default)]
pub struct DefaultLog {
    sink: MemorySink,
}

impl DefaultLog {
    pub fn logger(&self, name: &str) -> Logger {
        Logger::new(name, LogLevel::Info, Box::new(MemorySink::default()))
    }
    pub fn records(&self) -> &[LogRecord] {
        &self.sink.records
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_parse_round_trip() {
        assert_eq!(LogLevel::parse("warn"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse("bogus"), None);
        assert!(LogLevel::Trace < LogLevel::Fatal);
    }

    #[test]
    fn logger_filters_below_level() {
        let (shared, records) = SharedSink::new();
        let mut lg = Logger::new("pixfer", LogLevel::Info, Box::new(shared));
        lg.debug("below threshold");
        lg.info("visible");
        // Debug is below the Info threshold: only the info record lands.
        let levels: Vec<LogLevel> = records.lock().unwrap().iter().map(|r| r.level).collect();
        assert_eq!(levels, vec![LogLevel::Info]);
    }

    #[test]
    fn logger_emits_at_and_above_level() {
        let (shared, records) = SharedSink::new();
        let mut lg = Logger::new("rx", LogLevel::Info, Box::new(shared));
        lg.debug("below threshold");
        lg.info("visible");
        lg.warn("also visible");
        lg.error_with(
            "bad",
            &PixferError::Timeout {
                timeout_ms: 2500,
                chunk_idx: 1,
                attempts: 3,
            },
        );
        // debug filtered, info/warn/error emitted.
        let levels: Vec<LogLevel> = records.lock().unwrap().iter().map(|r| r.level).collect();
        assert_eq!(
            levels,
            vec![LogLevel::Info, LogLevel::Warn, LogLevel::Error]
        );
    }

    #[test]
    fn writer_sink_formats_records() {
        use std::io::Cursor;
        let mut buf = Cursor::new(Vec::new());
        let mut sink = WriterSink::new(&mut buf);
        let rec = LogRecord {
            level: LogLevel::Error,
            logger: "tx".into(),
            message: "chunk timed out".into(),
            context: None,
            error: Some(PixferError::Timeout {
                timeout_ms: 2500,
                chunk_idx: 2,
                attempts: 3,
            }),
        };
        sink.emit(&rec);
        let out = String::from_utf8(buf.into_inner()).unwrap();
        assert!(out.contains("ERROR"));
        assert!(out.contains("tx"));
        assert!(out.contains("chunk timed out"));
        assert!(out.contains("err="));
    }
}
