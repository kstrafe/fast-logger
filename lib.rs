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
//! # Example - Generic #
//!
//! Note that generic logging requires indirection at runtime, and may slow down your program.
//! Still, generic logging is very desirable because it is easy to use. There are two ways to do
//! generic logging depending on your needs:
//!
//! ```
//! use logger::{info, Generic, Logger};
//!
//! fn main() {
//!     let mut logger = Logger::<Generic>::spawn();
//!     info![logger, "context", "Message {}", "More"; "key" => "value", "three" => 3];
//! }
//! ```
//! The other macros are [trace!], [debug!], [warn!], [error!], and the generic [log!].
//!
//! If you wish to mix this with static logging, you can do the following:
//! ```
//! use logger::{info, Generic, Logger};
//!
//! enum MyMsg {
//!     Static(&'static str),
//!     Dynamic(Generic),
//! }
//!
//! impl std::fmt::Display for MyMsg {
//!     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//!         match self {
//!             MyMsg::Static(string) => write![f, "{}", string],
//!             MyMsg::Dynamic(handle) => handle.fmt(f),
//!         }
//!     }
//! }
//!
//! impl From<Generic> for MyMsg {
//!     fn from(f: Generic) -> Self {
//!         MyMsg::Dynamic(f)
//!     }
//! }
//!
//! fn main() {
//!     // Setup
//!     let mut logger = Logger::<MyMsg>::spawn();
//!     info![logger, "context", "Message {}", "More"; "key" => "value", "three" => 3];
//! }
//! ```
//!
//! # Example of static logging #
//!
//! Here is an example of static-logging only, the macros do not work for this, as these generate
//! a [Generic]. Anything implementing [Into] for the type of the logger will be accepted into
//! the logging functions.
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
//!     let mut logger = Logger::<MyMessageEnum>::spawn();
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
//! }
//! ```
//!
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
//!     let mut logger = Logger::<MyMessageEnum>::spawn();
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
//! }
//! ```
//!
//! # Example with just strings #
//!
//! If you really don't care about formatting overhead on the caller's side, you can just use a
//! [String] as the message type.
//!
//! ```
//! use logger::Logger;
//!
//! fn main() {
//!     // Setup
//!     let mut logger = Logger::<String>::spawn();
//!
//!     // Set the log level of `ctx` to 70, this filters
//!     // All future log levels 71-255 out.
//!     logger.set_context_specific_log_level("ctx", 70);
//!
//!     // This gets printed, because `warn` logs at level 64 <= 70
//!     logger.warn("ctx", format!("1"));
//!
//!     // This gets printed, because 50 <= 70
//!     logger.log(50, "ctx", format!("2"));
//!
//!     // This does not get printed, because !(80 <= 70)
//!     logger.log(80, "ctx", format!("3"));
//!
//!     // This gets printed, because the context is different
//!     logger.log(128, "ctx*", format!("4"));
//! }
//! ```
#![feature(test)]
extern crate test;

use chrono::prelude::*;
use colored::*;
use crossbeam_channel::{bounded, Receiver, RecvError, Sender, TrySendError};
use std::{
    collections::HashMap,
    fmt::{self, Debug, Display},
    marker::Send,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

/// The logger which dependent crates should use
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
    // Explicitly first so it's the first to drop
    log_channel: Sender<(u8, &'static str, C)>,
    context_specific_level: Arc<Mutex<HashMap<&'static str, u8>>>,
    level: Arc<AtomicUsize>,
    log_channel_full_count: Arc<AtomicUsize>,
    thread_handle: Arc<AutoJoinHandle>,
    colorize: Arc<AtomicBool>,
}

// ---

/// Wrapper for [JoinHandle], joins on [drop]
struct AutoJoinHandle {
    thread: Option<JoinHandle<()>>,
}

impl Drop for AutoJoinHandle {
    fn drop(&mut self) {
        self.thread.take().map(JoinHandle::join);
    }
}

// ---

/// A handle for generic logging data, used by macros
#[derive(Clone)]
pub struct Generic(Arc<dyn Fn(&mut fmt::Formatter) -> fmt::Result + Send + Sync>);

impl Display for Generic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self.0)(f)
    }
}

// ---

/// This is an implementation detail used by macros, and should NOT be called directly!
#[doc(hidden)]
pub fn make_generic__(
    arg: Arc<dyn Fn(&mut fmt::Formatter) -> fmt::Result + Send + Sync>,
) -> Generic {
    Generic(arg)
}

// ---

/// Equivalent to logging to the [log] function with an appropriate level, context, and a
/// [Generic].
#[macro_export]
macro_rules! log {
    ($n:expr, $log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*) => {{
        $log.log($n, $ctx, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            Ok({
                write![f, $fmt, $($msg),*]?;
                $(
                    write![f, ", {}={}", $key, $val]?;
                )*
            })
        })))
    }};
    ($n:expr, $log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*) => {{
        $log.log($n, $ctx, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            Ok({
                write![f, $fmt]?;
                $(
                    write![f, ", {}={}", $key, $val]?;
                )*
            })
        })))
    }};
    ($n:expr, $log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*) => {{
        $log.log($n, $ctx, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            write![f, $fmt, $($msg),*]
        })))
    }};
    ($n:expr, $log:expr, $ctx:expr, $fmt:expr) => {{
        $log.log($n, $ctx, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            write![f, $fmt]
        })))
    }};
}

/// Equivalent to [log!] with a level of 255
#[macro_export]
macro_rules! trace {
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*) => { $crate::log![255, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*) => { $crate::log![255, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*,) => { $crate::log![255, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*,) => { $crate::log![255, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*) => { $crate::log![255, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*,) => { $crate::log![255, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*) => { $crate::log![255, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,) => { $crate::log![255, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr) => { $crate::log![255, $log, $ctx, $fmt] };
}

/// Equivalent to [log!] with a level of 196
#[macro_export]
macro_rules! debug {
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*) => { $crate::log![192, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*) => { $crate::log![192, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*,) => { $crate::log![192, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*,) => { $crate::log![192, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*) => { $crate::log![192, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*,) => { $crate::log![192, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*) => { $crate::log![192, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,) => { $crate::log![192, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr) => { $crate::log![192, $log, $ctx, $fmt] };
}

/// Equivalent to [log!] with a level of 128
#[macro_export]
macro_rules! info {
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*) => { $crate::log![128, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*) => { $crate::log![128, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*,) => { $crate::log![128, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*,) => { $crate::log![128, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*) => { $crate::log![128, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*,) => { $crate::log![128, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*) => { $crate::log![128, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,) => { $crate::log![128, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr) => { $crate::log![128, $log, $ctx, $fmt] };
}

/// Equivalent to [log!] with a level of 64
#[macro_export]
macro_rules! warn {
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*) => { $crate::log![64, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*) => { $crate::log![64, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*,) => { $crate::log![64, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*,) => { $crate::log![64, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*) => { $crate::log![64, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*,) => { $crate::log![64, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*) => { $crate::log![64, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,) => { $crate::log![64, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr) => { $crate::log![64, $log, $ctx, $fmt] };
}

/// Equivalent to [log!] with a level of 0
#[macro_export]
macro_rules! error {
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*) => { $crate::log![0, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*) => { $crate::log![0, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,; $($key:expr => $val:expr),*,) => { $crate::log![0, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*; $($key:expr => $val:expr),*,) => { $crate::log![0, $log, $ctx, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*) => { $crate::log![0, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr; $($key:expr => $val:expr),*,) => { $crate::log![0, $log, $ctx, $fmt; $($key => $val),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*) => { $crate::log![0, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr, $($msg:expr),*,) => { $crate::log![0, $log, $ctx, $fmt, $($msg),*] };
    ($log:expr, $ctx:expr, $fmt:expr) => { $crate::log![0, $log, $ctx, $fmt] };
}

// ---

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
    pub fn spawn() -> Logger<C> {
        let (tx, rx) = bounded(CHANNEL_SIZE);
        let colorize = Arc::new(AtomicBool::new(false));
        let colorize_clone = colorize.clone();
        let full_count = Arc::new(AtomicUsize::new(0));
        let full_count_clone = full_count.clone();
        let level = Arc::new(AtomicUsize::new(DEFAULT_LEVEL as usize));
        let level_clone = level.clone();
        let ex = std::io::stdout();
        let context_specific_level = Arc::new(Mutex::new(HashMap::new()));
        let context_specific_level_clone = context_specific_level.clone();
        let logger_thread = thread::Builder::new()
            .name("logger".to_string())
            .spawn(move || {
                logger_thread(
                    rx,
                    full_count_clone,
                    ex.lock(),
                    context_specific_level_clone,
                    level_clone,
                    colorize_clone,
                )
            })
            .unwrap();
        Logger {
            thread_handle: Arc::new(AutoJoinHandle {
                thread: Some(logger_thread),
            }),
            log_channel: tx,
            log_channel_full_count: full_count,
            level,
            context_specific_level,
            colorize,
        }
    }

    /// Create a logger object with a specific writer
    ///
    /// See `spawn` for more information regarding spawning. This function providing the logger a
    /// writer, which makes the logger use any arbitrary writing interface.
    pub fn spawn_with_writer<T: 'static + std::io::Write + Send>(writer: T) -> Logger<C> {
        let (tx, rx) = bounded(CHANNEL_SIZE);
        let colorize = Arc::new(AtomicBool::new(false));
        let colorize_clone = colorize.clone();
        let full_count = Arc::new(AtomicUsize::new(0));
        let full_count_clone = full_count.clone();
        let level = Arc::new(AtomicUsize::new(DEFAULT_LEVEL as usize));
        let level_clone = level.clone();
        let context_specific_level = Arc::new(Mutex::new(HashMap::new()));
        let context_specific_level_clone = context_specific_level.clone();
        let logger_thread = thread::Builder::new()
            .name("logger".to_string())
            .spawn(move || {
                logger_thread(
                    rx,
                    full_count_clone,
                    writer,
                    context_specific_level_clone,
                    level_clone,
                    colorize_clone,
                )
            })
            .unwrap();
        Logger {
            thread_handle: Arc::new(AutoJoinHandle {
                thread: Some(logger_thread),
            }),
            log_channel: tx,
            log_channel_full_count: full_count,
            level,
            context_specific_level,
            colorize,
        }
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
    pub fn log(&mut self, level: u8, ctx: &'static str, message: impl Into<C>) -> bool {
        if level as usize <= self.level.load(Ordering::Relaxed) {
            match self.log_channel.try_send((level, ctx, message.into())) {
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
    pub fn trace(&mut self, _: &'static str, _: impl Into<C>) -> bool {
        false
    }

    /// Log an error message (level 255)
    ///
    /// Does nothing when compiled without debug assertions
    #[cfg(debug_assertions)]
    pub fn trace(&mut self, ctx: &'static str, message: impl Into<C>) -> bool {
        self.log(255, ctx, message)
    }

    /// Log an error message (level 192)
    pub fn debug(&mut self, ctx: &'static str, message: impl Into<C>) -> bool {
        self.log(192, ctx, message)
    }

    /// Log an error message (level 128)
    pub fn info(&mut self, ctx: &'static str, message: impl Into<C>) -> bool {
        self.log(128, ctx, message)
    }

    /// Log an error message (level 64)
    pub fn warn(&mut self, ctx: &'static str, message: impl Into<C>) -> bool {
        self.log(64, ctx, message)
    }

    /// Log an error message (level 0)
    pub fn error(&mut self, ctx: &'static str, message: impl Into<C>) -> bool {
        self.log(0, ctx, message)
    }

    pub fn set_colorize(&mut self, on: bool) {
        self.colorize.store(on, Ordering::Relaxed);
    }
}

impl<C: 'static + Display + Send + From<String>> LoggerV2Async<C> {
    pub fn make_writer(&mut self, ctx: &'static str, level: u8) -> impl std::fmt::Write + '_ {
        struct Writer<'a, C: Display + Send + From<String>> {
            logger: &'a mut Logger<C>,
            ctx: &'static str,
            level: u8,
        }
        impl<'a, C: 'static + Display + Send + From<String>> std::fmt::Write for Writer<'a, C> {
            fn write_str(&mut self, s: &str) -> Result<(), std::fmt::Error> {
                self.logger.log(self.level, self.ctx, s.to_string());
                Ok(())
            }
        }
        Writer {
            logger: self,
            ctx,
            level,
        }
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

fn colorize_level(level: u8) -> colored::ColoredString {
    if level < 64 {
        format!["{:03}", level].red()
    } else if level < 128 {
        format!["{:03}", level].yellow()
    } else if level < 192 {
        format!["{:03}", level].green()
    } else if level < 255 {
        format!["{:03}", level].cyan()
    } else if level == 255 {
        format!["{:03}", level].magenta()
    } else {
        unreachable![]
    }
}

fn do_write_nocolor<W: std::io::Write, T: Display>(
    writer: &mut W,
    lvl: u8,
    ctx: &'static str,
    msg: &str,
    now: T,
    newlines: usize,
    last_is_line: bool,
) -> std::io::Result<()> {
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

#[allow(clippy::too_many_arguments)]
fn do_write_color<W: std::io::Write, T: Display>(
    writer: &mut W,
    lvl: u8,
    ctx: &'static str,
    msg: &str,
    now: T,
    newlines: usize,
    last_is_line: bool,
    color_counter: &mut usize,
) -> std::io::Result<()> {
    *color_counter = (*color_counter + 1) % 2;
    let color;
    match color_counter {
        0 => color = "blue",
        1 => color = "magenta",
        _ => unimplemented![],
    }
    let msg_color;
    match color_counter {
        0 => msg_color = "bright blue",
        1 => msg_color = "bright magenta",
        _ => unimplemented![],
    }
    let msg = msg.color(color);
    if newlines > 1 {
        for (idx, line) in msg.lines().enumerate() {
            writeln![
                writer,
                "{}: {} {} {}: {}",
                now.to_string().color(color),
                colorize_level(lvl),
                ctx.bright_green(),
                format![
                    "[{:0width$}/{}]",
                    idx + 1,
                    newlines,
                    width = count_digits_base_10(newlines),
                ]
                .bright_yellow(),
                line.color(msg_color),
            ]?;
        }
        if last_is_line {
            writeln![
                writer,
                "{}: {} {} {}: ",
                now.to_string().color(color),
                colorize_level(lvl),
                ctx.bright_green(),
                format!["[{}/{}]", newlines, newlines,].bright_yellow(),
            ]?;
        }
    } else {
        writeln![
            writer,
            "{}: {} {}: {}",
            now.to_string().color(color),
            colorize_level(lvl),
            ctx.bright_green(),
            msg.color(msg_color),
        ]?;
    }
    Ok(())
}

fn do_write<C: Display + Send, W: std::io::Write>(
    writer: &mut W,
    lvl: u8,
    ctx: &'static str,
    msg: C,
    colorize: bool,
    color_counter: &mut usize,
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

    if colorize {
        do_write_color(
            writer,
            lvl,
            ctx,
            msg.as_str(),
            now,
            newlines,
            last_is_line,
            color_counter,
        )
    } else {
        do_write_nocolor(writer, lvl, ctx, msg.as_str(), now, newlines, last_is_line)
    }
}

fn logger_thread<C: Display + Send, W: std::io::Write>(
    rx: Receiver<(u8, &'static str, C)>,
    dropped: Arc<AtomicUsize>,
    mut writer: W,
    context_specific_level: Arc<Mutex<HashMap<&'static str, u8>>>,
    global_level: Arc<AtomicUsize>,
    colorize: Arc<AtomicBool>,
) {
    let mut color_counter = 0;
    let mut color;
    'outer_loop: loop {
        match rx.recv() {
            Ok(msg) => {
                color = colorize.load(Ordering::Relaxed);
                let lvls = context_specific_level.lock();
                match lvls {
                    Ok(lvls) => {
                        if let Some(lvl) = lvls.get(msg.1) {
                            if msg.0 <= *lvl
                                && do_write(
                                    &mut writer,
                                    msg.0,
                                    msg.1,
                                    msg.2,
                                    color,
                                    &mut color_counter,
                                )
                                .is_err()
                            {
                                break 'outer_loop;
                            }
                        } else if do_write(
                            &mut writer,
                            msg.0,
                            msg.1,
                            msg.2,
                            color,
                            &mut color_counter,
                        )
                        .is_err()
                        {
                            break 'outer_loop;
                        }
                    }
                    Err(_poison) => {
                        let _ = do_write(
                            &mut writer,
                            0,
                            "logger",
                            "Context specific level lock has been poisoned. Exiting logger",
                            color,
                            &mut color_counter,
                        );
                        break 'outer_loop;
                    }
                }
            }
            Err(error @ RecvError { .. }) => {
                color = colorize.load(Ordering::Relaxed);
                let lvls = context_specific_level.lock();
                match lvls {
                    Ok(lvls) => {
                        if let Some(lvl) = lvls.get("logger") {
                            if 196 <= *lvl && 196 <= global_level.load(Ordering::Relaxed) {
                                let _ = do_write(
                                    &mut writer,
                                    196,
                                    "logger",
                                    format![
                                        "Unable to receive message. Exiting logger, reason={}",
                                        error
                                    ],
                                    color,
                                    &mut color_counter,
                                );
                            }
                        } else if 196 <= global_level.load(Ordering::Relaxed) {
                            let _ = do_write(
                                &mut writer,
                                196,
                                "logger",
                                format![
                                    "Unable to receive message. Exiting logger, reason={}",
                                    error
                                ],
                                color,
                                &mut color_counter,
                            );
                        }
                    }
                    Err(_poison) => {
                        // Do nothing
                    }
                }
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
                color,
                &mut color_counter,
            )
            .is_err()
        {
            break 'outer_loop;
        }
    }
}

// ---

pub struct InDebug<'a, T: Debug>(pub &'a T);

impl<'a, T: Debug> Display for InDebug<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        ((self.0) as &Debug).fmt(f)
    }
}

pub struct InDebugPretty<'a, T: Debug>(pub &'a T);

impl<'a, T: Debug> Display for InDebugPretty<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write![f, "{:#?}", self.0]
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
    fn send_simple_string() {
        use std::fmt::Write;
        let mut logger = Logger::<String>::spawn();
        assert_eq![true, logger.info("tst", "Message")];
        let mut writer = logger.make_writer("tst*", 128);
        write![writer, "Message 2"].unwrap();
        std::mem::drop(writer);
    }

    #[test]
    fn send_successful_message() {
        let mut logger = Logger::<Log>::spawn();
        assert_eq![true, logger.info("tst", Log::Static("Message"))];
    }

    #[test]
    fn trace_is_disabled_by_default() {
        let mut logger = Logger::<Log>::spawn();
        assert_eq![false, logger.trace("tst", Log::Static("Message"))];
    }

    #[test]
    fn debug_is_disabled_by_default() {
        let mut logger = Logger::<Log>::spawn();
        assert_eq![false, logger.debug("tst", Log::Static("Message"))];
    }

    #[test]
    fn info_is_enabled_by_default() {
        let mut logger = Logger::<Log>::spawn();
        assert_eq![true, logger.info("tst", Log::Static("Message"))];
    }

    #[test]
    fn warn_is_enabled_by_default() {
        let mut logger = Logger::<Log>::spawn();
        assert_eq![true, logger.warn("tst", Log::Static("Message"))];
    }

    #[test]
    fn error_is_enabled_by_default() {
        let mut logger = Logger::<Log>::spawn();
        assert_eq![true, logger.error("tst", Log::Static("Message"))];
    }

    #[test]
    fn custom_writer() {
        let writer = Void {};
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message"))];
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
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message"))];
        assert_eq![true, logger.error("tst", Log::Static("Second message"))];
        let regex = Regex::new(
            r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst: Message\n",
        )
        .unwrap();
        std::mem::drop(logger);
        assert![regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap())];
    }

    #[test]
    fn ensure_ending_message_when_exit_1() {
        let store = Arc::new(Mutex::new(vec![]));
        let writer = Store {
            store: store.clone(),
        };
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        logger.set_log_level(196);
        let regex = Regex::new(
            r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 196 logger: Unable to receive message. Exiting logger, reason=receiving on an empty and disconnected channel\n$",
        )
        .unwrap();
        std::mem::drop(logger);
        assert![regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap())];
    }

    #[test]
    fn ensure_ending_message_when_exit_2() {
        let store = Arc::new(Mutex::new(vec![]));
        let writer = Store {
            store: store.clone(),
        };
        let logger = Logger::<Log>::spawn_with_writer(writer);
        let regex = Regex::new(r"^$").unwrap();
        std::mem::drop(logger);
        assert![regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap())];
    }

    #[test]
    fn ensure_ending_message_when_exit_3() {
        let store = Arc::new(Mutex::new(vec![]));
        let writer = Store {
            store: store.clone(),
        };
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        logger.set_log_level(196);
        logger.set_context_specific_log_level("logger", 195);
        let regex = Regex::new(r"^$").unwrap();
        std::mem::drop(logger);
        assert![regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap())];
    }

    #[test]
    fn ensure_proper_message_format_line_ending_with_newline() {
        let store = Arc::new(Mutex::new(vec![]));
        let writer = Store {
            store: store.clone(),
        };
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message\n"))];
        let regex = Regex::new(
            r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[1/2\]: Message
(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[2/2\]: \n",
        )
        .unwrap();
        std::mem::drop(logger);
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
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message\nPart\n2"))];
        let regex = Regex::new(
            r#"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[1/3\]: Message\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[2/3\]: Part\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[3/3\]: 2\n"#,
        )
        .unwrap();
        std::mem::drop(logger);
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
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        assert_eq![true, logger.error("tst", Log::Static("Message\nPart\n2\n"))];
        let regex = Regex::new(
            r#"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[1/4\]: Message\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[2/4\]: Part\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[3/4\]: 2\n(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{1,2} \d+ \d{2}:\d{2}:\d{2}.\d{9}(\+|-)\d{4}: 000 tst \[4/4\]: \n"#,
        )
        .unwrap();
        std::mem::drop(logger);
        assert![
            regex.is_match(&String::from_utf8(store.lock().unwrap().to_vec()).unwrap()),
            String::from_utf8(store.lock().unwrap().to_vec()).unwrap()
        ];
    }

    #[test]
    fn generic() {
        let mut logger = Logger::<Generic>::spawn();
        log![123, logger, "tst", "lorem {}", "ipsum"; "dolor" => "sit", "amet" => 1234];
        trace![logger, "tst", "lorem {}", "ipsum"; "a" => "b"];
        debug![logger, "tst", "lorem {}", "ipsum"; "a" => "b"];
        info![logger, "tst", "lorem {}", "ipsum"; "a" => "b"];
        warn![logger, "tst", "lorem {}", "ipsum"; "a" => "b"];
        error![logger, "tst", "lorem {}", "ipsum"; "a" => "b"];

        let ipsum = 1;
        info![logger, "tst", "lorem {}", ipsum; "dolor" => "sit"];
    }

    #[test]
    fn custom_writer_with_generic() {
        impl From<Generic> for Log {
            fn from(_: Generic) -> Self {
                Log::Static("Unable to convert")
            }
        }
        let mut logger = Logger::<Log>::spawn();
        assert_eq![true, logger.error("tst", Log::Static("Message"))];
        assert_eq![true, error![logger, "tst", "Message"]];
    }

    #[rustfmt::skip]
    #[test]
    fn ensure_all_macro_variants_can_be_used() {
        let mut logger = Logger::<Log>::spawn();

        assert_eq![false, trace![logger, "tst", "Message"]];
        assert_eq![false, trace![logger, "tst", "Message",]];
        assert_eq![false, trace![logger, "tst", "Message {}", "argument"]];
        assert_eq![false, trace![logger, "tst", "Message {}", "argument",]];
        assert_eq![false, trace![logger, "tst", "Message";]];
        assert_eq![false, trace![logger, "tst", "Message"; "a" => "b"]];
        assert_eq![false, trace![logger, "tst", "Message"; "a" => "b",]];
        assert_eq![false, trace![logger, "tst", "Message",;]];
        assert_eq![false, trace![logger, "tst", "Message",; "a" => "b"]];
        assert_eq![false, trace![logger, "tst", "Message",; "a" => "b",]];
        assert_eq![false, trace![logger, "tst", "Message {}", "argument";]];
        assert_eq![false, trace![logger, "tst", "Message {}", "argument"; "a" => "b"]];
        assert_eq![false, trace![logger, "tst", "Message {}", "argument"; "a" => "b",]];
        assert_eq![false, trace![logger, "tst", "Message {}", "argument",;]];
        assert_eq![false, trace![logger, "tst", "Message {}", "argument",; "a" => "b"]];
        assert_eq![false, trace![logger, "tst", "Message {}", "argument",; "a" => "b",]];

        assert_eq![false, debug![logger, "tst", "Message"]];
        assert_eq![false, debug![logger, "tst", "Message",]];
        assert_eq![false, debug![logger, "tst", "Message {}", "argument"]];
        assert_eq![false, debug![logger, "tst", "Message {}", "argument",]];
        assert_eq![false, debug![logger, "tst", "Message";]];
        assert_eq![false, debug![logger, "tst", "Message"; "a" => "b"]];
        assert_eq![false, debug![logger, "tst", "Message"; "a" => "b",]];
        assert_eq![false, debug![logger, "tst", "Message",;]];
        assert_eq![false, debug![logger, "tst", "Message",; "a" => "b"]];
        assert_eq![false, debug![logger, "tst", "Message",; "a" => "b",]];
        assert_eq![false, debug![logger, "tst", "Message {}", "argument";]];
        assert_eq![false, debug![logger, "tst", "Message {}", "argument"; "a" => "b"]];
        assert_eq![false, debug![logger, "tst", "Message {}", "argument"; "a" => "b",]];
        assert_eq![false, debug![logger, "tst", "Message {}", "argument",;]];
        assert_eq![false, debug![logger, "tst", "Message {}", "argument",; "a" => "b"]];
        assert_eq![false, debug![logger, "tst", "Message {}", "argument",; "a" => "b",]];

        assert_eq![true, info![logger, "tst", "Message"]];
        assert_eq![true, info![logger, "tst", "Message",]];
        assert_eq![true, info![logger, "tst", "Message {}", "argument"]];
        assert_eq![true, info![logger, "tst", "Message {}", "argument",]];
        assert_eq![true, info![logger, "tst", "Message";]];
        assert_eq![true, info![logger, "tst", "Message"; "a" => "b"]];
        assert_eq![true, info![logger, "tst", "Message"; "a" => "b",]];
        assert_eq![true, info![logger, "tst", "Message",;]];
        assert_eq![true, info![logger, "tst", "Message",; "a" => "b"]];
        assert_eq![true, info![logger, "tst", "Message",; "a" => "b",]];
        assert_eq![true, info![logger, "tst", "Message {}", "argument";]];
        assert_eq![true, info![logger, "tst", "Message {}", "argument"; "a" => "b"]];
        assert_eq![true, info![logger, "tst", "Message {}", "argument"; "a" => "b",]];
        assert_eq![true, info![logger, "tst", "Message {}", "argument",;]];
        assert_eq![true, info![logger, "tst", "Message {}", "argument",; "a" => "b"]];
        assert_eq![true, info![logger, "tst", "Message {}", "argument",; "a" => "b",]];

        assert_eq![true, warn![logger, "tst", "Message"]];
        assert_eq![true, warn![logger, "tst", "Message",]];
        assert_eq![true, warn![logger, "tst", "Message {}", "argument"]];
        assert_eq![true, warn![logger, "tst", "Message {}", "argument",]];
        assert_eq![true, warn![logger, "tst", "Message";]];
        assert_eq![true, warn![logger, "tst", "Message"; "a" => "b"]];
        assert_eq![true, warn![logger, "tst", "Message"; "a" => "b",]];
        assert_eq![true, warn![logger, "tst", "Message",;]];
        assert_eq![true, warn![logger, "tst", "Message",; "a" => "b"]];
        assert_eq![true, warn![logger, "tst", "Message",; "a" => "b",]];
        assert_eq![true, warn![logger, "tst", "Message {}", "argument";]];
        assert_eq![true, warn![logger, "tst", "Message {}", "argument"; "a" => "b"]];
        assert_eq![true, warn![logger, "tst", "Message {}", "argument"; "a" => "b",]];
        assert_eq![true, warn![logger, "tst", "Message {}", "argument",;]];
        assert_eq![true, warn![logger, "tst", "Message {}", "argument",; "a" => "b"]];
        assert_eq![true, warn![logger, "tst", "Message {}", "argument",; "a" => "b",]];

        assert_eq![true, error![logger, "tst", "Message"]];
        assert_eq![true, error![logger, "tst", "Message",]];
        assert_eq![true, error![logger, "tst", "Message {}", "argument"]];
        assert_eq![true, error![logger, "tst", "Message {}", "argument",]];
        assert_eq![true, error![logger, "tst", "Message";]];
        assert_eq![true, error![logger, "tst", "Message"; "a" => "b"]];
        assert_eq![true, error![logger, "tst", "Message"; "a" => "b",]];
        assert_eq![true, error![logger, "tst", "Message",;]];
        assert_eq![true, error![logger, "tst", "Message",; "a" => "b"]];
        assert_eq![true, error![logger, "tst", "Message",; "a" => "b",]];
        assert_eq![true, error![logger, "tst", "Message {}", "argument";]];
        assert_eq![true, error![logger, "tst", "Message {}", "argument"; "a" => "b"]];
        assert_eq![true, error![logger, "tst", "Message {}", "argument"; "a" => "b",]];
        assert_eq![true, error![logger, "tst", "Message {}", "argument",;]];
        assert_eq![true, error![logger, "tst", "Message {}", "argument",; "a" => "b"]];
        assert_eq![true, error![logger, "tst", "Message {}", "argument",; "a" => "b",]];
    }

    #[test]
    fn colorize() {
        let mut logger = Logger::<Log>::spawn();
        logger.set_log_level(255);
        logger.set_colorize(true);
        logger.trace("tst", Log::Static("A trace message"));
        logger.debug("tst", Log::Static("A debug message"));
        logger.info("tst", Log::Static("An info message"));
        logger.warn("tst", Log::Static("A warning message"));
        logger.error("tst", Log::Static("An error message"));

        logger.info("tst", Log::Static("On\nmultiple\nlines\n"));
    }

    #[test]
    fn using_indebug() {
        let mut logger = Logger::<Log>::spawn();
        #[derive(Clone)]
        struct Value {}
        impl std::fmt::Debug for Value {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write![f, "Debug Value"]
            }
        }
        let value = Value {};
        info![logger, "tst", "Message"; "value" => InDebug(&value)];
        let value = Value {};
        info![logger, "tst", "Message"; "value" => InDebugPretty(&value)];
    }

    #[test]
    fn indebug() {
        assert_eq!["[1, 2, 3]", format!["{}", InDebug(&[1, 2, 3])]];
        assert_eq![
            "[\n    1,\n    2,\n    3\n]",
            format!["{}", InDebugPretty(&[1, 2, 3])]
        ];
    }

    // ---

    #[bench]
    fn sending_a_message_to_trace_default(b: &mut Bencher) {
        let mut logger = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.trace("tst", black_box(Log::Static("Message"))));
        });
    }

    #[bench]
    fn sending_a_message_to_debug_default(b: &mut Bencher) {
        let mut logger = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.debug("tst", black_box(Log::Static("Message"))));
        });
    }

    #[bench]
    fn sending_a_message_to_info_default(b: &mut Bencher) {
        let mut logger = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.info("tst", black_box(Log::Static("Message"))));
        });
    }

    #[bench]
    fn sending_a_complex_message_trace(b: &mut Bencher) {
        let mut logger = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.trace("tst", black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))))
        });
    }

    #[bench]
    fn sending_a_complex_message_info(b: &mut Bencher) {
        let mut logger = Logger::<Log>::spawn();
        b.iter(|| {
            black_box(logger.info("tst", black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
    }

    // ---

    #[bench]
    fn custom_writer_sending_a_message_to_trace_default(b: &mut Bencher) {
        let writer = Void {};
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        assert![!logger.trace(
            "tst",
            Log::Static("Trace should be disabled during benchmark (due to compiler optimization)")
        )];
        b.iter(|| {
            black_box(logger.trace("tst", black_box(Log::Static("Message"))));
        });
    }

    #[bench]
    fn custom_writer_sending_a_message_to_debug_default(b: &mut Bencher) {
        let writer = Void {};
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        assert![!logger.debug(
            "tst",
            Log::Static(
                "Debug should be disabled during benchmark (due to the standard log level)"
            )
        )];
        b.iter(|| {
            black_box(logger.debug("tst", black_box(Log::Static("Message"))));
        });
    }

    #[bench]
    fn custom_writer_sending_a_message_to_info_default(b: &mut Bencher) {
        let writer = Void {};
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        b.iter(|| {
            black_box(logger.info("tst", black_box(Log::Static("Message"))));
        });
    }

    #[bench]
    fn custom_writer_sending_a_complex_message_trace(b: &mut Bencher) {
        let writer = Void {};
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        assert![!logger.trace(
            "tst",
            Log::Static("Trace should be disabled during benchmark")
        )];
        b.iter(|| {
            black_box(logger.trace("tst", black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))))
        });
    }

    #[bench]
    fn custom_writer_sending_a_complex_message_info(b: &mut Bencher) {
        let writer = Void {};
        let mut logger = Logger::<Log>::spawn_with_writer(writer);
        b.iter(|| {
            black_box(logger.info("tst", black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
    }

    #[bench]
    fn using_macros_to_send_message(b: &mut Bencher) {
        let writer = Void {};
        let mut logger = Logger::<Generic>::spawn_with_writer(writer);
        b.iter(|| {
            black_box(info!(logger, "tst", "Message {:?}", black_box(&[1, 2, 3]); black_box("pi") => black_box(3.14)));
        });
    }

    #[bench]
    fn custom_writer_sending_a_complex_format_message_info(b: &mut Bencher) {
        let writer = Void {};
        let mut logger = Logger::<String>::spawn_with_writer(writer);
        b.iter(|| {
            black_box(logger.info(
                "tst",
                black_box(format!["Message {} {:?}", 3.14, &[1, 2, 3]]),
            ));
        });
    }

    #[test]
    fn void_logger_speed_info_with_macros() {
        // NOTE: This "benchmark" is implemented as a test because benches tend to
        // overflow the channel
        let writer = Void {};
        let mut logger = Logger::<Generic>::spawn_with_writer(writer);
        let before = std::time::Instant::now();
        for _ in 0..CHANNEL_SIZE {
            black_box(
                info!(logger, black_box("tst"), "Message {:?}", black_box(&[1, 2, 3]); black_box("pi") => black_box(3.14)),
            );
        }
        let after = std::time::Instant::now();
        println![
            "void logger speed info with macros: {:?}",
            (after - before) / (CHANNEL_SIZE as u32)
        ];
    }

    #[test]
    fn void_logger_speed_info_without_macros() {
        // NOTE: This "benchmark" is implemented as a test because benches tend to
        // overflow the channel
        let writer = Void {};
        let mut logger = Logger::<usize>::spawn_with_writer(writer);
        let before = std::time::Instant::now();
        for _ in 0..CHANNEL_SIZE {
            logger.info("tst", black_box(12345usize));
        }
        let after = std::time::Instant::now();
        println![
            "void logger speed info: {:?}",
            (after - before) / (CHANNEL_SIZE as u32)
        ];
    }

    #[test]
    fn void_logger_speed_debug() {
        // NOTE: This "benchmark" is implemented as a test because benches tend to
        // overflow the channel
        let writer = Void {};
        let mut logger = Logger::<usize>::spawn_with_writer(writer);
        let before = std::time::Instant::now();
        for _ in 0..CHANNEL_SIZE {
            logger.debug("tst", black_box(12345usize));
        }
        let after = std::time::Instant::now();
        println![
            "void logger speed debug: {:?}",
            (after - before) / (CHANNEL_SIZE as u32)
        ];
    }

    #[test]
    fn void_logger_speed_trace() {
        // NOTE: This "benchmark" is implemented as a test because benches tend to
        // overflow the channel
        let writer = Void {};
        let mut logger = Logger::<usize>::spawn_with_writer(writer);
        let before = std::time::Instant::now();
        for _ in 0..CHANNEL_SIZE {
            logger.trace("tst", black_box(12345usize));
        }
        let after = std::time::Instant::now();
        println![
            "void logger speed trace: {:?}",
            (after - before) / (CHANNEL_SIZE as u32)
        ];
    }

    // ---

    use slog::{o, Drain, Level, OwnedKVList, Record};
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
        let drain = slog_async::Async::new(drain)
            .chan_size(CHANNEL_SIZE)
            .build()
            .fuse();
        let log = slog::Logger::root(drain, o![]);
        assert![!log.is_trace_enabled()];
        b.iter(|| {
            black_box(slog::trace![log, "{}", black_box("Message")]);
        });
    }

    #[bench]
    fn message_slog_disabled_dynamically_on_async_side(b: &mut Bencher) {
        let decorator = slog_term::PlainDecorator::new(Void {});
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog::LevelFilter::new(drain, Level::Critical).fuse();
        let drain = slog_async::Async::new(drain)
            .chan_size(CHANNEL_SIZE)
            .build()
            .fuse();
        let log = slog::Logger::root(drain, o![]);
        assert![log.is_info_enabled()];
        b.iter(|| {
            black_box(slog::info![log, "{}", black_box("Message")]);
        });
    }

    #[bench]
    fn message_slog_disabled_dynamically_on_caller_side(b: &mut Bencher) {
        let decorator = slog_term::PlainDecorator::new(Void {});
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain)
            .chan_size(CHANNEL_SIZE)
            .build()
            .fuse();
        let drain = slog::LevelFilter::new(drain, Level::Critical).fuse();
        let log = slog::Logger::root(drain, o![]);
        assert![!log.is_info_enabled()];
        b.iter(|| {
            black_box(slog::info![log, "{}", black_box("Message")]);
        });
    }

    #[bench]
    fn message_slog_enabled(b: &mut Bencher) {
        let decorator = slog_term::PlainDecorator::new(Void {});
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain)
            .chan_size(CHANNEL_SIZE)
            .build()
            .fuse();
        let log = slog::Logger::root(drain, o![]);
        b.iter(|| {
            black_box(slog::info![log, "{}", black_box("Message")]);
        });
    }
}
