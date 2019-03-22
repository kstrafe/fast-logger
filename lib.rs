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
//! use logger::Logger;
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
//! # Why the logging functions are not generic #
//! Why aren't the logging functions generic over `Display`?
//! Because that would require a reference to be sent to a `&'static Display`, which is hard to do
//! when this `Display` is built from a string read from a socket. This is because we need to - at
//! compile time - give the channel a type so that it can see the size of the type.
//! # Example with log levels #
//!
//! Here is an example where we set a context specific log level.
//!
//! ```
//! use logger::Logger;
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
//!     // Set the log level of `ctx` to 70, this filters
//!     // All future log levels 71-255 out.
//!     logger.set_context_specific_log_level("ctx", 70);
//!
//!     // This gets printed, because `warn` logs at level 64 <= 70
//!     logger.warn("ctx", MyMessageEnum::SimpleMessage("1"));
//!
//!     // This gets printed, because 50 <= 70
//!     logger.log(50, "ctx", MyMessageEnum::SimpleMessage("2"));
//!
//!     // This does not get printed, because !(80 <= 70)
//!     logger.log(80, "ctx", MyMessageEnum::SimpleMessage("3"));
//!
//!     // This gets printed, because the context is different
//!     logger.log(128, "ctx*", MyMessageEnum::SimpleMessage("4"));
//!
//!     // Teardown
//!     std::mem::drop(logger);
//!     thread.join().unwrap();
//! }
//! ```
#![feature(test)]
extern crate test;

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
            thread::Builder::new()
                .name("logger".to_string())
                .spawn(move || logger_thread(rx, full_count, ex.lock(), context_specific_level))
                .unwrap(),
        )
    }

    /// Create a logger object with a specific writer
    ///
    /// See `spawn` for more information regarding spawning. This function providing the logger a
    /// writer, which makes the logger use any arbitrary writing interface.
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
            thread::Builder::new()
                .name("logger".to_string())
                .spawn(move || logger_thread(rx, full_count, writer, context_specific_level))
                .unwrap(),
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

    /// Retrieve the current global log level value
    pub fn get_log_level(&self) -> u8 {
        self.level.load(Ordering::Relaxed) as u8
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

fn count_digits_base_10(mut number: usize) -> usize {
    let mut digits = 1;
    while number >= 10 {
        number /= 10;
        digits += 1;
    }
    digits
}

fn do_write<C: Display + Send, W: std::io::Write>(
    writer: &mut W,
    lvl: u8,
    ctx: &'static str,
    msg: C,
) -> std::io::Result<()> {
    const ITEMS: &[chrono::format::Item] = {
        use chrono::format::{Fixed::*, Item::*, Numeric::*, Pad::*};
        &[
            Fixed(ShortMonthName),
            Literal(" "),
            Numeric(Day, None),
            Literal(" "),
            Numeric(Year, None),
            Literal(" "),
            Numeric(Hour, Zero),
            Literal(":"),
            Numeric(Minute, Zero),
            Literal(":"),
            Numeric(Second, Zero),
            Fixed(Nanosecond9),
            Fixed(TimezoneOffset),
        ]
    };
    let now = Local::now().format_with_items(ITEMS.iter().cloned());
    let msg = format!["{}", msg];

    let mut newlines = 1;
    let mut last_is_line = false;
    for ch in msg.chars() {
        if ch == '\n' {
            newlines += 1;
            last_is_line = true;
        } else {
            last_is_line = false;
        }
    }

    if newlines > 1 {
        for (idx, line) in msg.lines().enumerate() {
            writeln![
                writer,
                "{}: {:03} {} [{:0width$}/{}]: {}",
                now,
                lvl,
                ctx,
                idx + 1,
                newlines,
                line,
                width = count_digits_base_10(newlines),
            ]?;
        }
        if last_is_line {
            writeln![
                writer,
                "{}: {:03} {} [{}/{}]: ",
                now, lvl, ctx, newlines, newlines
            ]?;
        }
    } else {
        writeln![writer, "{}: {:03} {}: {}", now, lvl, ctx, msg,]?;
    }
    Ok(())
}

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
                            if msg.0 <= *lvl && do_write(&mut writer, msg.0, msg.1, msg.2).is_err()
                            {
                                break 'outer_loop;
                            }
                        } else if do_write(&mut writer, msg.0, msg.1, msg.2).is_err() {
                            break 'outer_loop;
                        }
                    }
                    Err(_poison) => {
                        let _ = do_write(
                            &mut writer,
                            0,
                            "logger",
                            "Context specific level lock has been poisoned. Exiting logger",
                        );
                        break 'outer_loop;
                    }
                }
            }
            Err(error @ RecvError { .. }) => {
                let _ = do_write(
                    &mut writer,
                    0,
                    "logger",
                    format![
                        "Unable to receive message. Exiting logger, reason={}",
                        error
                    ],
                );
                break 'outer_loop;
            }
        }
        let dropped_messages = dropped.swap(0, Ordering::Relaxed);
        if dropped_messages > 0
            && do_write(
                &mut writer,
                64,
                "logger",
                format![
                    "Logger dropped messages due to channel overflow, count={}",
                    dropped_messages
                ],
            )
            .is_err()
        {
            break 'outer_loop;
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
    fn count_digits() {
        for i in 0..10 {
            assert_eq![1, count_digits_base_10(i)];
        }
        for i in 10..100 {
            assert_eq![2, count_digits_base_10(i)];
        }
        for i in &[100usize, 123, 321, 980] {
            assert_eq![3, count_digits_base_10(*i)];
        }
        assert_eq![4, count_digits_base_10(1248)];
        assert_eq![10, count_digits_base_10(01329583467)];
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
            r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst: Message\n",
        )
        .unwrap();
        assert![regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap())];
    }

    #[test]
    fn ensure_proper_message_format_line_ending_with_newline() {
        let store = Arc::new(Mutex::new(vec![]));
        let writer = Store {
            store: store.clone(),
        };
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message\n"))];
        std::mem::drop(logger);
        thread.join().unwrap();
        let regex = Regex::new(
            r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[1/2\]: Message
(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[2/2\]: \n",
        )
        .unwrap();
        assert![
            regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap()),
            String::from_utf8(store.lock().unwrap().to_vec()).unwrap()
        ];
    }

    #[test]
    fn multiple_lines_count_correctly() {
        let store = Arc::new(Mutex::new(vec![]));
        let writer = Store {
            store: store.clone(),
        };
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message\nPart\n2"))];
        std::mem::drop(logger);
        thread.join().unwrap();
        let regex = Regex::new(
            r#"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[1/3\]: Message\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[2/3\]: Part\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[3/3\]: 2\n"#,
        )
        .unwrap();
        assert![
            regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap()),
            String::from_utf8(store.lock().unwrap().to_vec()).unwrap()
        ];
    }

    #[test]
    fn multiple_lines_count_correctly_trailing() {
        let store = Arc::new(Mutex::new(vec![]));
        let writer = Store {
            store: store.clone(),
        };
        let (mut logger, thread) = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message\nPart\n2\n"))];
        std::mem::drop(logger);
        thread.join().unwrap();
        let regex = Regex::new(
            r#"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[1/4\]: Message\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[2/4\]: Part\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[3/4\]: 2\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[4/4\]: \n"#,
        )
        .unwrap();
        assert![
            regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap()),
            String::from_utf8(store.lock().unwrap().to_vec()).unwrap()
        ];
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
