/// Equivalent to logging to the [crate::LoggerV2Async::log] function with an appropriate level, context, and a
/// [Generic].
#[macro_export]
macro_rules! log {
    ($n:expr, $log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => {{
        $(
            let $cl = $cl.clone();
        )*
        $log.log($n, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            Ok({
                write![f, $fmt, $($msg),*]?;
                $(
                    write![f, ", {}={}", $key, $val]?;
                )*
            })
        })))
    }};
    ($n:expr, $log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => {{
        $(
            let $cl = $cl.clone();
        )*
        $log.log($n, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            Ok({
                write![f, $fmt]?;
                $(
                    write![f, ", {}={}", $key, $val]?;
                )*
            })
        })))
    }};
    ($n:expr, $log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?) => {{
        $log.log($n, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            Ok({
                write![f, $fmt, $($msg),*]?;
                $(
                    write![f, ", {}={}", $key, $val]?;
                )*
            })
        })))
    }};
    ($n:expr, $log:expr, $fmt:expr; $($key:expr => $val:expr),* $(,)?) => {{
        $log.log($n, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            Ok({
                write![f, $fmt]?;
                $(
                    write![f, ", {}={}", $key, $val]?;
                )*
            })
        })))
    }};
    ($n:expr, $log:expr, $fmt:expr, $($msg:expr),* $(,)?) => {{
        $log.log($n, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            write![f, $fmt, $($msg),*]
        })))
    }};
    ($n:expr, $log:expr, $fmt:expr $(,)?) => {{
        $log.log($n, $crate::make_generic__(::std::sync::Arc::new(move |f| -> ::std::fmt::Result {
            write![f, $fmt]
        })))
    }};
}

/// Equivalent to [log!] with a level of 255
#[macro_export]
macro_rules! trace {
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![255, $log, $fmt, $($msg),*; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![255, $log, $fmt; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![255, $log, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![255, $log, $fmt; $($key => $val),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?) => { $crate::log![255, $log, $fmt, $($msg),*] };
    ($log:expr, $fmt:expr $(,)?) => { $crate::log![255, $log, $fmt] };
}

/// Equivalent to [log!] with a level of 192
#[macro_export]
macro_rules! debug {
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![192, $log, $fmt, $($msg),*; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![192, $log, $fmt; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![192, $log, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![192, $log, $fmt; $($key => $val),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?) => { $crate::log![192, $log, $fmt, $($msg),*] };
    ($log:expr, $fmt:expr $(,)?) => { $crate::log![192, $log, $fmt] };
}

/// Equivalent to [log!] with a level of 128
#[macro_export]
macro_rules! info {
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![128, $log, $fmt, $($msg),*; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![128, $log, $fmt; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![128, $log, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![128, $log, $fmt; $($key => $val),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?) => { $crate::log![128, $log, $fmt, $($msg),*] };
    ($log:expr, $fmt:expr $(,)?) => { $crate::log![128, $log, $fmt] };
}

/// Equivalent to [log!] with a level of 64
#[macro_export]
macro_rules! warn {
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![64, $log, $fmt, $($msg),*; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![64, $log, $fmt; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![64, $log, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![64, $log, $fmt; $($key => $val),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?) => { $crate::log![64, $log, $fmt, $($msg),*] };
    ($log:expr, $fmt:expr $(,)?) => { $crate::log![64, $log, $fmt] };
}

/// Equivalent to [log!] with a level of 0
#[macro_export]
macro_rules! error {
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![0, $log, $fmt, $($msg),*; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?; clone $($cl:ident),* $(,)?) => { $crate::log![0, $log, $fmt; $($key => $val),*; clone $($cl),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![0, $log, $fmt, $($msg),*; $($key => $val),*] };
    ($log:expr, $fmt:expr $(,)?; $($key:expr => $val:expr),* $(,)?) => { $crate::log![0, $log, $fmt; $($key => $val),*] };
    ($log:expr, $fmt:expr, $($msg:expr),* $(,)?) => { $crate::log![0, $log, $fmt, $($msg),*] };
    ($log:expr, $fmt:expr $(,)?) => { $crate::log![0, $log, $fmt] };
}
