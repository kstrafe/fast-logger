//! A threaded logger
//!
//! The logger described in this file uses no global state, it all goes through
//! the glocal `universe::glocals::Threads`.
//!
//! When we create a logger, a thread is spawned and a bounded buffer is allocated
//! for messages to be sent to the logger. This is also accompanied by a counter for
//! failed messages - when the buffer is full, the message is discarded and the counter
//! incremented. Such a counter is useful so the logger can report how many messages
//! were dropped.
//!
//! For using the logger, all you need to use is the `log` function.
//! It takes a logging level and log context, which are used to limit messages
//! if needed. The log levels range from 0 to 255, where 0 will _always_ be logged
//! (unless the queue is full). The context ought to be a short descriptor
//! of where this log message came from the semantic sense.
//!
//! ```
//! use universe::{glocals::Threads, mediators::logger::{create_logger, log}};
//! fn main() {
//!     // Allocate the structure for storing threads from `universe`
//!     let mut threads = Threads::default();
//!
//!     // Start the logger thread, storing the context and queue inside the `threads` structure
//!     create_logger(&mut threads);
//!
//!     // Log a message by pushing it to the logger's queue
//!     // Returning true if there was an active queue with sufficient space
//!     // and false if the message could not be sent.
//!     assert![
//!         log(
//!             // Threads variable so we can communicate to the logger thread
//!             &mut threads,
//!
//!             // The logging level
//!             128,
//!
//!             // The logging context
//!             "TEST",
//!
//!             // An arbitrary message describing the event
//!             "This message does not arrive, and the failed count will _not_ be incremented",
//!
//!             // A key-value map of items, also printed by the logger
//!             // Mainly useful when reporting state
//!             &[("key", "value")]
//!         )
//!     ];
//!
//!     // Close the logging channel, thus signalling to the logger thread that
//!     // we are finished
//!     threads.log_channel = None;
//!
//!     // Join the logger thread with this one
//!     threads.logger.unwrap().join();
//! }
//! ```
use chrono::prelude::*;
use std::{
    fmt,
    fmt::Display,
    marker::{Send, Sync},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, RecvError, TrySendError},
        Arc, Mutex,
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
