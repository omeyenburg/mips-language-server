//! Macros for quick logging.
//!
//! By default, use the eprintln! macro for logging.
//! If the "log-file" feature is enabled, a log will be written
//! to lsp.log in the current directory using tracing.

#[macro_export]
#[cfg(not(feature = "log-file"))]
macro_rules! log {
    ($($arg:tt)*) => {
        eprintln!($($arg)*)
    };
}

#[macro_export]
#[cfg(not(feature = "log-file"))]
macro_rules! log_init {
    () => {};
}

#[macro_export]
#[cfg(feature = "log-file")]
macro_rules! log {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*)
    };
}

#[macro_export]
#[cfg(feature = "log-file")]
macro_rules! log_init {
    () => {
        let log_file = std::fs::File::create("./lsp.log").expect("Failed to create logging file.");
        let log_file = std::io::BufWriter::new(log_file);
        let (non_blocking, _guard) = tracing_appender::non_blocking(log_file);
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::level_filters::LevelFilter::DEBUG)
            .with_writer(non_blocking)
            .without_time() // Compact log messages
            .with_level(false)
            .with_target(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Could not set default subscriber");
    };
}
