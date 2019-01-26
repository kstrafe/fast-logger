//! A simple log-level based logger
//!
//! # General #
//!
//! When spawning a logger, a thread is created and a handle structure returned. This handle can be
//! copied and shared among threads. All it does is hold some mutexes and atomic references to the
//! logger. Actually logging pushes the data over an asynchronous channel with size limits.
//!
//! The logger requires a `Display` type to be provided so the logger can actually print data. The
//! reason for this is that it cuts down on serialization cost for the caller, leaving the logger
//! to serialize numbers into strings and perform other formatting work.
//!
//! # Log Levels #
//!
//! Logger features two log level controls: per-context and "global". When logging a message, the
//! global log level is checked, and if the current message has a lower priority than the global
//! log level, nothing will be sent to the logger.
//!
//! Once the logger has received the message, it checks its internal mapping from context to log
//! level, if the message's log level has a lower priority than the context log level, it is
//! dropped.
//!
//! We normally use the helper functions `trace`, `debug`, `info`, `warn`, and `error`, which have
//! the corresponding log levels: 255, 192, 128, 64, 0, respectively.
//!
//! Trace is disabled with debug_assertions off.
//!
//! Note: an `error` message priority 0, and log levels are always unsigned, so an `error` message
//! can never be filtered.
//!
//! # Example #
//!
//! Here is an example:
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
//!     logger.info("ctx", MyMessageEnum::SimpleMessage("Hello world!"));
//!
//!     // Various logging levels
//!     logger.trace("ctx", MyMessageEnum::SimpleMessage("Hello world!"));
//!     logger.debug("ctx", MyMessageEnum::SimpleMessage("Hello world!"));
//!     logger.info("ctx", MyMessageEnum::SimpleMessage("Hello world!"));
//!     logger.warn("ctx", MyMessageEnum::SimpleMessage("Hello world!"));
//!     logger.error("ctx", MyMessageEnum::SimpleMessage("Hello world!"));
//!
//!     // Teardown
//!     std::mem::drop(logger);
//!     thread.join().unwrap();
//! }
//! ```
use chrono::prelude::*;
use std::{
    collections::HashMap,
    fmt::Display,
    marker::Send,
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
#[derive(Clone)]
pub struct LoggerV2Async<C: Display + Send> {
    log_channel: mpsc::SyncSender<(u8, &'static str, C)>,
    log_channel_full_count: Arc<AtomicUsize>,
    level: Arc<AtomicUsize>,
    context_specific_level: Arc<Mutex<HashMap<&'static str, u8>>>,
}

static CHANNEL_SIZE: usize = 30_000;
static DEFAULT_LEVEL: u8 = 128;

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
        let (tx, rx) = mpsc::sync_channel(CHANNEL_SIZE);
        let full_count = Arc::new(AtomicUsize::new(0));
        let level = Arc::new(AtomicUsize::new(DEFAULT_LEVEL as usize));
        let ex = std::io::stdout();
        let context_specific_level = Arc::new(Mutex::new(HashMap::new()));
        (
            Logger {
                log_channel: tx,
                log_channel_full_count: full_count.clone(),
                level,
                context_specific_level: context_specific_level.clone(),
            },
            thread::spawn(move || logger_thread(rx, full_count, ex.lock(), context_specific_level)),
        )
    }

    pub fn spawn_with_writer<T: 'static + std::io::Write + Send>(
        writer: T,
    ) -> (Logger<C>, thread::JoinHandle<()>) {
        let (tx, rx) = mpsc::sync_channel(CHANNEL_SIZE);
        let full_count = Arc::new(AtomicUsize::new(0));
        let level = Arc::new(AtomicUsize::new(DEFAULT_LEVEL as usize));
        let context_specific_level = Arc::new(Mutex::new(HashMap::new()));
        (
            Logger {
                log_channel: tx,
                log_channel_full_count: full_count.clone(),
                level,
                context_specific_level: context_specific_level.clone(),
            },
            thread::spawn(move || logger_thread(rx, full_count, writer, context_specific_level)),
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

    /// Sets the log level for a specific context
    ///
    /// Whenever the logger receives a message, it will use the context-to-level
    /// mapping to see if the message should be logged or not.
    /// Note that this happens after checking the global log level.
    pub fn set_context_specific_log_level(&mut self, ctx: &'static str, level: u8) {
        if let Ok(ref mut lvl) = self.context_specific_level.lock() {
            lvl.insert(ctx, level);
        }
    }

    /// Generic logging function
    ///
    /// Send a message with a specific log-level.
    pub fn log(&mut self, level: u8, ctx: &'static str, message: C) -> bool {
        if level as usize <= self.level.load(Ordering::Relaxed) {
            match self.log_channel.try_send((level, ctx, message)) {
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
    ///
    /// Does nothing when compiled without debug assertions
    #[cfg(not(debug_assertions))]
    pub fn trace(&mut self, _: &'static str, _: C) -> bool {
        false
    }

    /// Log an error message (level 255)
    ///
    /// Does nothing when compiled without debug assertions
    #[cfg(debug_assertions)]
    pub fn trace(&mut self, ctx: &'static str, message: C) -> bool {
        self.log(255, ctx, message)
    }

    /// Log an error message (level 192)
    pub fn debug(&mut self, ctx: &'static str, message: C) -> bool {
        self.log(192, ctx, message)
    }

    /// Log an error message (level 128)
    pub fn info(&mut self, ctx: &'static str, message: C) -> bool {
        self.log(128, ctx, message)
    }

    /// Log an error message (level 64)
    pub fn warn(&mut self, ctx: &'static str, message: C) -> bool {
        self.log(64, ctx, message)
    }

    /// Log an error message (level 0)
    pub fn error(&mut self, ctx: &'static str, message: C) -> bool {
        self.log(0, ctx, message)
    }
}

// ---

fn logger_thread<C: Display + Send, W: std::io::Write>(
    rx: mpsc::Receiver<(u8, &'static str, C)>,
    dropped: Arc<AtomicUsize>,
    mut writer: W,
    context_specific_level: Arc<Mutex<HashMap<&'static str, u8>>>,
) {
    'outer_loop: loop {
        match rx.recv() {
            Ok(msg) => {
                let lvls = context_specific_level.lock();
                match lvls {
                    Ok(lvls) => {
                        if let Some(lvl) = lvls.get(msg.1) {
                            if msg.0 <= *lvl {
                                if writeln![
                                    writer,
                                    "{}: {:03} [{}]: {}",
                                    Local::now(),
                                    msg.0,
                                    msg.1,
                                    msg.2
                                ]
                                .is_err()
                                {
                                    break 'outer_loop;
                                }
                            }
                        } else {
                            if writeln![
                                writer,
                                "{}: {:03} [{}]: {}",
                                Local::now(),
                                msg.0,
                                msg.1,
                                msg.2
                            ]
                            .is_err()
                            {
                                break 'outer_loop;
                            }
                        }
                    }
                    Err(_poison) => {
                        let _ = writeln![writer, "{}: {:03} [{}]: Context specific level lock has been poisoned. Exiting logger", Local::now(), 0, "logger"];
                        break 'outer_loop;
                    }
                }
            }
            Err(error @ RecvError { .. }) => {
                let _ = writeln![
                    writer,
                    "{}: {:03} [{}]: Unable to receive message. Exiting logger, reason={}",
                    Local::now(),
                    128,
                    "logger",
                    error
                ];
                break 'outer_loop;
            }
        }
        let dropped_messages = dropped.swap(0, Ordering::Relaxed);
        if dropped_messages > 0 {
            if writeln![
                writer,
                "{}: {:03} [{}]: {}, {}={}",
                Local::now(),
                64,
                "logger",
                "logger dropped messages due to channel overflow",
                "count",
                dropped_messages
            ]
            .is_err()
            {
                break 'outer_loop;
            };
        }
    }
}

// ---

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::{fmt, io};
    use test::{black_box, Bencher};

    // ---

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

    struct Void {}
    impl io::Write for Void {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    // ---

    struct Store {
        pub store: Arc<Mutex<Vec<u8>>>,
    }
    impl io::Write for Store {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut vector = self.store.lock().unwrap();
            vector.extend(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    // ---

    #[test]
    fn send_successful_message() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![true, logger.info("tst", Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn trace_is_disabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![false, logger.trace("tst", Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn debug_is_disabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![false, logger.debug("tst", Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn info_is_enabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![true, logger.info("tst", Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn warn_is_enabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![true, logger.warn("tst", Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn error_is_enabled_by_default() {
        let (mut logger, thread) = Logger::<Log>::spawn();
        assert_eq![true, logger.error("tst", Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn custom_writer() {
        let writer = Void {};
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[test]
    fn ensure_proper_message_format() {
        let store = Arc::new(Mutex::new(vec![]));
        let writer = Store {
            store: store.clone(),
        };
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message"))];
        assert_eq![true, logger.error("tst", Log::Static("Second message"))];
        std::mem::drop(logger);
        thread.join().unwrap();
        let regex = Regex::new(
            r"^\d+-\d{2}-\d{2} \d{2}:\d{2}:\d{2}.\d{9} (\+|-)\d\d:\d\d: 000 \[tst\]: Message\n",
        )
        .unwrap();
        assert![regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap())];
    }

    // ---

    #[bench]
    fn sending_a_message_to_trace_default(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.trace("tst", black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[bench]
    fn sending_a_message_to_debug_default(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.debug("tst", black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[bench]
    fn sending_a_message_to_info_default(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.info("tst", black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[bench]
    fn sending_a_complex_message_trace(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.trace("tst", black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))))
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[bench]
    fn sending_a_complex_message_info(b: &mut Bencher) {
        let (mut logger, thread) = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.info("tst", black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    // ---

    #[bench]
    fn custom_writer_sending_a_message_to_trace_default(b: &mut Bencher) {
        let writer = Void {};
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        assert![!logger.trace(
            "tst",
            Log::Static("Trace should be disabled during benchmark (due to compiler optimization)")
        )];
        b.iter(|| {
            black_box(logger.trace("tst", black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[bench]
    fn custom_writer_sending_a_message_to_debug_default(b: &mut Bencher) {
        let writer = Void {};
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        assert![!logger.debug(
            "tst",
            Log::Static(
                "Debug should be disabled during benchmark (due to the standard log level)"
            )
        )];
        b.iter(|| {
            black_box(logger.debug("tst", black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[bench]
    fn custom_writer_sending_a_message_to_info_default(b: &mut Bencher) {
        let writer = Void {};
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        b.iter(|| {
            black_box(logger.info("tst", black_box(Log::Static("Message"))));
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[bench]
    fn custom_writer_sending_a_complex_message_trace(b: &mut Bencher) {
        let writer = Void {};
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        assert![!logger.trace(
            "tst",
            Log::Static("Trace should be disabled during benchmark")
        )];
        b.iter(|| {
            black_box(logger.trace("tst", black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))))
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    #[bench]
    fn custom_writer_sending_a_complex_message_info(b: &mut Bencher) {
        let writer = Void {};
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        b.iter(|| {
            black_box(logger.info("tst", black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
        std::mem::drop(logger);
        thread.join().unwrap();
    }

    // ---

    use slog::{info, o, trace, Drain, Level, OwnedKVList, Record};
    impl Drain for Void {
        type Ok = ();
        type Err = ();
        fn log(&self, _: &Record, _: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
            Ok(())
        }
    }

    #[bench]
    fn message_slog_disabled_by_compiler(b: &mut Bencher) {
        let decorator = slog_term::PlainDecorator::new(Void {});
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        let log = slog::Logger::root(drain, o![]);
        assert![!log.is_trace_enabled()];
        b.iter(|| {
            black_box(trace![log, "{}", black_box("Message")]);
        });
    }

    #[bench]
    fn message_slog_disabled_dynamically_on_async_side(b: &mut Bencher) {
        let decorator = slog_term::PlainDecorator::new(Void {});
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog::LevelFilter::new(drain, Level::Critical).fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        let log = slog::Logger::root(drain, o![]);
        assert![log.is_info_enabled()];
        b.iter(|| {
            black_box(info![log, "{}", black_box("Message")]);
        });
    }

    #[bench]
    fn message_slog_disabled_dynamically_on_caller_side(b: &mut Bencher) {
        let decorator = slog_term::PlainDecorator::new(Void {});
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        let drain = slog::LevelFilter::new(drain, Level::Critical).fuse();
        let log = slog::Logger::root(drain, o![]);
        assert![!log.is_info_enabled()];
        b.iter(|| {
            black_box(info![log, "{}", black_box("Message")]);
        });
    }

    #[bench]
    fn message_slog_enabled(b: &mut Bencher) {
        let decorator = slog_term::PlainDecorator::new(Void {});
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        let log = slog::Logger::root(drain, o![]);
        b.iter(|| {
            black_box(info![log, "{}", black_box("Message")]);
        });
    }
}
