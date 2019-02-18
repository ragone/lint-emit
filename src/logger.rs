use slog::{Level, Logger, Drain, info, o};
use slog_term::{TermDecorator, CompactFormat};

/// Get the logger based on verbosity
pub fn setup_logger(verbosity: u64) -> Logger {
    // Setup logging level
    let min_log_level = match verbosity {
        0 => Level::Error,   // Events that might still allow the application to continue running.
        1 => Level::Warning, // Potentially harmful situations.
        2 => Level::Info,    // Informational messages that highlight the progress of the application at coarse-grained level.
        3 => Level::Debug,   // Fine-grained informational events that are most useful to debug an application.
        _ => Level::Trace,   // Finer-grained informational events than DEBUG.
    };

    // Create logger
    let decorator = TermDecorator::new().build();
    let drain = CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = Logger::root(drain.filter_level(min_log_level).fuse(), o!());
    info!(logger, "{:#?} logging enabled", min_log_level);
    logger
}
