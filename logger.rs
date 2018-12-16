//! A simple log-level based logger
//!
//! ```
//! use universe::libs::logger::Logger;
//!
//! // You need to define your own message type
//! enum MyMessageEnum {
//!     SimpleMessage(&'static str)
//! }
//!
//! // It needs to implement std::fmt::Display
//! impl std::fmt::Display for MyMessageEnum {
//!     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//!         match self {
//!             MyMessageEnum::SimpleMessage(string) => write![f, "{}", string],
//!         }
//!     }
//! }
//!
//! fn main() {
//!     // Setup
//!     let (mut logger, thread) = Logger::spawn();
//!
//!     // Actual logging
//!     logger.info(MyMessageEnum::SimpleMessage("Hello world!"));
//!
//!     // Teardown
//!     std::mem::drop(logger);
//!     thread.join().unwrap();
//! }
//! ```
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
    /// the logger thread. The logger thread is in the background,
    /// waiting for messages to print out. Once all logger objects
    /// are dropped, the thread will die.
    ///
    /// Typically, you only call `spawn` once in an application
    /// since you just want a single logging thread to print stuff.
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

    /// The log-level is an 8-bit variable that is shared among
    /// all clones of this logger. When we try logging we first
    /// check if our message has a priority higher or equal to
    /// this level. If it doesn't we just exit the logger function.
    ///
    /// Internally, we use an atomic variable to store the logging
    /// level so all threads can check it quickly. This makes
    /// log statements that won't trigger because of the log-level
    /// absolutely fucking NUTS (so cheap it's basically
    /// non-existent).
    pub fn set_log_level(&mut self, level: u8) {
        self.level.store(level as usize, Ordering::Relaxed);
    }

    /// Generic logging function
    ///
    /// Send a message with a specific log-level.
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

    /// Log an error message (level 255)
    #[cfg(not(debug_assertions))]
    pub fn trace(&mut self, message: C) -> bool {
        false
    }

    /// Log an error message (level 255)
    #[cfg(debug_assertions)]
    pub fn trace(&mut self, message: C) -> bool {
        self.log(255, message)
    }

    /// Log an error message (level 192)
    pub fn debug(&mut self, message: C) -> bool {
        self.log(192, message)
    }

    /// Log an error message (level 128)
    pub fn info(&mut self, message: C) -> bool {
        self.log(128, message)
    }

    /// Log an error message (level 64)
    pub fn warn(&mut self, message: C) -> bool {
        self.log(64, message)
    }

    /// Log an error message (level 0)
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
                if writeln![writer, "{}: {:03}: {}", Local::now(), msg.0, msg.1].is_err() {
                    break;
                }
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

// ---

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use test::{black_box, Bencher};

    enum Log {
        Static(&'static str),
        Complex(&'static str, f32, &'static [u8]),
    }

    impl Display for Log {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Log::Static(str) => write![f, "{}", str],
                Log::Complex(str, value, list) => {
                    write![f, "{} {}", str, value]?;
                    for i in list.iter() {
                        write![f, "{}", i]?;
                    }
                    Ok(())
                }
            }
        }
    }

    // ---

    #[test]
    fn send_successful_message() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![true, logger.info(Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn trace_is_disabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![false, logger.trace(Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn debug_is_disabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![false, logger.debug(Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn info_is_enabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![true, logger.info(Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn warn_is_enabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![true, logger.warn(Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn error_is_enabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![true, logger.error(Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    // ---

    #[bench]
    fn sending_a_message_to_trace_default(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.trace(black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join();
    }

    #[bench]
    fn sending_a_message_to_debug_default(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.trace(black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join();
    }

    #[bench]
    fn sending_a_message_to_info_default(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.info(black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join();
    }

    #[bench]
    fn sending_a_complex_message_trace(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| black_box(logger.trace(black_box(Log::Complex("Message", 3.14, &[1, 2, 3])))));
        std::mem::drop(logger);
        thread.join();
    }

    #[bench]
    fn sending_a_complex_message_info(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.info(black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
        std::mem::drop(logger);
        thread.join();
    }
}
