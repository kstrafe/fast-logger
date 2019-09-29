use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fast_logger::{info, Generic, Logger, CHANNEL_SIZE};
use std::fmt;

// ---

enum Log {
    Static(&'static str),
    Complex(&'static str, f32, &'static [u8]),
    Generic(Generic),
}

impl fmt::Display for Log {
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
            Log::Generic(generic) => generic.fmt(f),
        }
    }
}

impl From<Generic> for Log {
    fn from(generic: Generic) -> Self {
        Log::Generic(generic)
    }
}

// ---

fn benchmark(c: &mut Criterion) {
    c.bench_function("sending message to trace default", |b| {
        let mut logger = Logger::<Log>::spawn("tst");
        b.iter(|| {
            black_box(logger.trace(black_box(Log::Static("Message"))));
        });
    });

    c.bench_function("sending message to debug default", |b| {
        let mut logger = Logger::<Log>::spawn("tst");
        b.iter(|| {
            black_box(logger.debug(black_box(Log::Static("Message"))));
        });
    });

    c.bench_function("sending message to info default", |b| {
        let mut logger = Logger::<Log>::spawn("tst");
        b.iter(|| {
            black_box(logger.info(black_box(Log::Static("Message"))));
        });
    });

    c.bench_function("sending message to complex trace", |b| {
        let mut logger = Logger::<Log>::spawn("tst");
        b.iter(|| {
            black_box(logger.trace(black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
    });

    c.bench_function("sending message to complex info", |b| {
        let mut logger = Logger::<Log>::spawn("tst");
        b.iter(|| {
            black_box(logger.info(black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
    });

    c.bench_function("void writer trace", |b| {
        let mut logger = Logger::<Log>::spawn_void();
        b.iter(|| {
            black_box(logger.trace(black_box(Log::Static("Message"))));
        });
    });

    c.bench_function("void writer debug", |b| {
        let mut logger = Logger::<Log>::spawn_void();
        b.iter(|| {
            black_box(logger.debug(black_box(Log::Static("Message"))));
        });
    });

    c.bench_function("void writer info", |b| {
        let mut logger = Logger::<Log>::spawn_void();
        b.iter(|| {
            black_box(logger.info(black_box(Log::Static("Message"))));
        });
    });

    c.bench_function("void writer trace complex", |b| {
        let mut logger = Logger::<Log>::spawn_void();
        b.iter(|| {
            black_box(logger.trace(black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
    });

    c.bench_function("void writer info complex", |b| {
        let mut logger = Logger::<Log>::spawn_void();
        b.iter(|| {
            black_box(logger.info(black_box(Log::Complex("Message", 3.14, &[1, 2, 3]))));
        });
    });

    c.bench_function("sending message to complex info (macro)", |b| {
        let mut logger = Logger::<Log>::spawn_void();
        b.iter(|| {
            black_box(info!(logger, "Message {:?}", black_box(&[1, 2, 3]); black_box("pi") => black_box(3.14)));
        });
    });
}

// ---

fn benchmark_slog(c: &mut Criterion) {
    use slog::{o, Drain, Level, OwnedKVList, Record};
    use std::io::Write;
    struct Void {}
    impl Drain for Void {
        type Ok = ();
        type Err = ();
        fn log(&self, _: &Record, _: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
            Ok(())
        }
    }
    impl Write for Void {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    c.bench_function("message slog disabled", |b| {
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
    });

    c.bench_function("message slog disabled dynamically (async)", |b| {
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
    });

    c.bench_function("message slog disabled dynamically (caller)", |b| {
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
    });

    c.bench_function("message slog enabled", |b| {
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
    });
}

// ---

criterion_group! {
    name = benches;
    config = Criterion::default().measurement_time(std::time::Duration::new(0, 100_000_000));
    targets = benchmark, benchmark_slog,
}
criterion_main!(benches);
