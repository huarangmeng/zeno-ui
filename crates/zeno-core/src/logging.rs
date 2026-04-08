use std::sync::Once;

pub use tracing as __private_tracing;
use tracing_subscriber::EnvFilter;

const ZENO_LOG_ENV: &str = "ZENO_LOG";
static LOGGING_INITIALIZED: Once = Once::new();

#[doc(hidden)]
pub fn __ensure_logging() {
    LOGGING_INITIALIZED.call_once(init_logging_internal);
}

#[macro_export]
macro_rules! zeno_trace {
    ($($arg:tt)*) => {
        {
            $crate::__ensure_logging();
            $crate::__private_tracing::trace!($($arg)*)
        }
    };
}

#[macro_export]
macro_rules! zeno_debug {
    ($($arg:tt)*) => {
        {
            $crate::__ensure_logging();
            $crate::__private_tracing::debug!($($arg)*)
        }
    };
}

#[macro_export]
macro_rules! zeno_info {
    ($($arg:tt)*) => {
        {
            $crate::__ensure_logging();
            $crate::__private_tracing::info!($($arg)*)
        }
    };
}

#[macro_export]
macro_rules! zeno_warn {
    ($($arg:tt)*) => {
        {
            $crate::__ensure_logging();
            $crate::__private_tracing::warn!($($arg)*)
        }
    };
}

#[macro_export]
macro_rules! zeno_error {
    ($($arg:tt)*) => {
        {
            $crate::__ensure_logging();
            $crate::__private_tracing::error!($($arg)*)
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! zeno_error_event {
    ($event:literal, $($arg:tt)*) => {{
        $crate::zeno_error!(target: "zeno.error", event = $event, $($arg)*);
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! zeno_warn_event {
    ($event:literal, $($arg:tt)*) => {{
        $crate::zeno_warn!(target: "zeno.warn", event = $event, $($arg)*);
    }};
}

#[macro_export]
macro_rules! zeno_warn_error {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_warn_event!(
            $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_error_error {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_error_event!(
            $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_backend_warn {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_warn!(
            target: "zeno.backend",
            event = $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_backend_error {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_error!(
            target: "zeno.backend",
            event = $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_session_warn {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_warn!(
            target: "zeno.session",
            event = $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_session_error {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_error!(
            target: "zeno.session",
            event = $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_window_warn {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_warn!(
            target: "zeno.window",
            event = $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_window_error {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_error!(
            target: "zeno.window",
            event = $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_runtime_warn {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_warn!(
            target: "zeno.runtime",
            event = $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_runtime_error {
    ($event:literal, $error:expr $(, $($arg:tt)*)?) => {{
        let error = &$error;
        $crate::zeno_error!(
            target: "zeno.runtime",
            event = $event,
            error_code = %error.error_code(),
            component = error.component(),
            op = error.operation(),
            error_kind = error.error_kind(),
            message = %error.message()
            $(, $($arg)*)?
        );
    }};
}

#[macro_export]
macro_rules! zeno_frame_log {
    (trace, $($arg:tt)*) => {
        $crate::zeno_trace!(target: "zeno.frame", $($arg)*)
    };
    (debug, $($arg:tt)*) => {
        $crate::zeno_debug!(target: "zeno.frame", $($arg)*)
    };
    (info, $($arg:tt)*) => {
        $crate::zeno_info!(target: "zeno.frame", $($arg)*)
    };
    (warn, $($arg:tt)*) => {
        $crate::zeno_warn!(target: "zeno.frame", $($arg)*)
    };
    (error, $($arg:tt)*) => {
        $crate::zeno_error!(target: "zeno.frame", $($arg)*)
    };
}

#[macro_export]
macro_rules! zeno_session_log {
    (trace, $($arg:tt)*) => {
        $crate::zeno_trace!(target: "zeno.session", $($arg)*)
    };
    (debug, $($arg:tt)*) => {
        $crate::zeno_debug!(target: "zeno.session", $($arg)*)
    };
    (info, $($arg:tt)*) => {
        $crate::zeno_info!(target: "zeno.session", $($arg)*)
    };
    (warn, $($arg:tt)*) => {
        $crate::zeno_warn!(target: "zeno.session", $($arg)*)
    };
    (error, $($arg:tt)*) => {
        $crate::zeno_error!(target: "zeno.session", $($arg)*)
    };
}

#[macro_export]
macro_rules! zeno_runtime_log {
    (trace, $($arg:tt)*) => {
        $crate::zeno_trace!(target: "zeno.runtime", $($arg)*)
    };
    (debug, $($arg:tt)*) => {
        $crate::zeno_debug!(target: "zeno.runtime", $($arg)*)
    };
    (info, $($arg:tt)*) => {
        $crate::zeno_info!(target: "zeno.runtime", $($arg)*)
    };
    (warn, $($arg:tt)*) => {
        $crate::zeno_warn!(target: "zeno.runtime", $($arg)*)
    };
    (error, $($arg:tt)*) => {
        $crate::zeno_error!(target: "zeno.runtime", $($arg)*)
    };
}

fn init_logging_internal() {
    let filter = build_env_filter();

    let _ = if cfg!(debug_assertions) {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .without_time()
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .pretty()
            .try_init()
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .without_time()
            .with_target(true)
            .compact()
            .try_init()
    };
}

fn build_env_filter() -> EnvFilter {
    let resolved = std::env::var(ZENO_LOG_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("RUST_LOG")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| default_filter().to_string());

    EnvFilter::try_new(resolved).unwrap_or_else(|_| EnvFilter::new(default_filter()))
}

fn default_filter() -> &'static str {
    if is_test_environment() {
        "off"
    } else if cfg!(debug_assertions) {
        "info,zeno=debug,zeno.frame=trace"
    } else {
        "info"
    }
}

fn is_test_environment() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(|value| value.to_owned()))
        .map_or(false, |path| path.contains("/deps/"))
}
