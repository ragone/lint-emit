#[macro_use] extern crate clap;
extern crate slog;
extern crate slog_term;
extern crate slog_async;
extern crate lint_forge;
extern crate itertools;
extern crate walkdir;
extern crate tui;
extern crate termion;

use clap::App;
use slog::{Level, Logger, Drain, info, debug, o};
use slog_term::{TermDecorator, CompactFormat};
use failure::Error;

fn main() -> Result<(), Error> {
    let yml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yml).get_matches();

    // Setup logging level
    let min_log_level = match matches.occurrences_of("verbose") {
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

    // Get commit range from args or default to HEAD
    let commit_range = matches.value_of("commit_range").unwrap();
    debug!(logger, "Commit Range = {:#?}", commit_range);

    // Get the linters from args or default to [clippy]
    let linters: Vec<_> = matches.values_of("linters").unwrap().collect();
    debug!(logger, "Linters = {:#?}", linters);

    lint_forge::run(commit_range, linters, logger)
}
