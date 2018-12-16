//! An asynchronous logger
use chrono::prelude::*;
use std::{
    fmt::Display,
    marker::Send,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, RecvError, TrySendError},
        Arc,
    },
    thread,
};

pub type Logger<C> = LoggerV2Async<C>;

/// The fastest logger in the west
///
/// A very simple logger that uses a custom structure and
/// a logging level. No further assumptions are made about
/// the structure or nature of the logging message, thus
/// making it extremely cheap to send messages to the logger
/// thread.
pub struct LoggerV2Async<C: Display + Send> {
    log_channel: mpsc::SyncSender<(u8, C)>,
    log_channel_full_count: Arc<AtomicUsize>,
    level: Arc<AtomicUsize>,
}

impl<C: 'static + Display + Send> LoggerV2Async<C> {
    /// Create a logger object and spawn a logging thread
    ///
    /// The logger object is the interface to write stuff to
    /// the logger. The logger thread is in the background,
    /// waiting for messages to print out. Once all logger objects
    /// are dropped, the thread will die.
    pub fn spawn() -> (Logger<C>, thread::JoinHandle<()>) {
        let (tx, rx) = mpsc::sync_channel(30_000);
        let full_count = Arc::new(AtomicUsize::new(0));
        let level = Arc::new(AtomicUsize::new(128));
        let ex = std::io::stdout();
        (
            Logger {
                log_channel: tx,
                log_channel_full_count: full_count.clone(),
                level,
            },
            thread::spawn(move || logger_thread(rx, full_count, ex)),
        )
    }

    pub fn set_log_level(&mut self, level: u8) {
        self.level.store(level as usize, Ordering::Relaxed);
    }

    pub fn log(&mut self, level: u8, message: C) -> bool {
        if level as usize <= self.level.load(Ordering::Relaxed) {
            match self.log_channel.try_send((level, message)) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => {
                    self.log_channel_full_count.fetch_add(1, Ordering::Relaxed);
                    false
                }
                Err(TrySendError::Disconnected(_)) => false,
            }
        } else {
            false
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn trace(&mut self, message: C) -> bool {
        false
    }

    #[cfg(debug_assertions)]
    pub fn trace(&mut self, message: C) -> bool {
        self.log(255, message)
    }

    pub fn debug(&mut self, message: C) -> bool {
        self.log(192, message)
    }

    pub fn info(&mut self, message: C) -> bool {
        self.log(128, message)
    }

    pub fn warn(&mut self, message: C) -> bool {
        self.log(64, message)
    }

    pub fn error(&mut self, message: C) -> bool {
        self.log(0, message)
    }
}

// ---

fn logger_thread<C: Display + Send, W: std::io::Write>(
    rx: mpsc::Receiver<(u8, C)>,
    dropped: Arc<AtomicUsize>,
    mut writer: W,
) {
    loop {
        match rx.recv() {
            Ok(msg) => {
                writeln![writer, "{}: {:03}: {}", Local::now(), msg.0, msg.1];
            }
            Err(RecvError { .. }) => {
                break;
            }
        }
        let dropped_messages = dropped.swap(0, Ordering::Relaxed);
        if dropped_messages > 0 {
            println![
                "{}: {:03}: {}, {}={}",
                Local::now(),
                0,
                "logger dropped messages due to channel overflow",
                "count",
                dropped_messages
            ];
        }
    }
}

