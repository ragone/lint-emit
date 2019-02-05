#[macro_use] extern crate clap;
extern crate slog;
extern crate slog_term;
extern crate slog_async;
extern crate lint_forge;
extern crate itertools;
extern crate walkdir;
extern crate tui;
extern crate termion;

mod display;

use clap::App;
use slog::{Level, Logger, Drain, info, debug, o};
use slog_term::{TermDecorator, CompactFormat};
use failure::Error;
use lint_forge::*;
use indicatif::{ProgressBar};
use rayon::prelude::*;
use colored::*;

fn main() -> Result<(), Error> {
    let yml = load_yaml!("cli.yml");
    let linters_yml = load_yaml!("linters.yml");
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
    let linters: Vec<&str> = matches.values_of("linters").unwrap().collect();
    debug!(logger, "Linters = {:#?}", linters);

    // Create LinterConfigs for linters
    let linter_configs: Vec<LinterConfig> = linters
        .into_iter()
        .map(|linter| {
            let config = &linters_yml[linter];
            let cmd = config["cmd"].as_str().unwrap().to_owned();
            let regex = config["regex"].as_str().unwrap().to_owned();
            LinterConfig {
                name: linter.to_owned(),
                cmd,
                regex,
            }
        })
        .collect();

    run(commit_range, linter_configs, logger)
}

/// Run the linters across the whole project and return the linting messages
/// for just the changed lines
fn run(commit_range: &str, linters: Vec<LinterConfig>, logger: slog::Logger) -> Result<(), Error> {
    // Get the changed files
    let changed_files = get_changed_files(commit_range)?;
    debug!(logger, "Changed Files = {:#?}", changed_files);

    // Get the changed files and line numbers
    let diff_metas: Vec<DiffMeta> = changed_files
        .par_iter()
        .map(|file| get_changed_lines(commit_range, &file).unwrap())
        .collect();
    debug!(logger, "Diff Metas = {:#?}", diff_metas);

    let pb = ProgressBar::new(diff_metas.len() as u64);

    // Get the output from running the linters for each file
    let lint_messages: Vec<LintMessage> = diff_metas
        .par_iter()
        .flat_map(|diff_meta| {
            let lint_messages = get_lint_messages(&linters, &diff_meta, &logger);
            match lint_messages {
                Ok(lint_messages) => {
                    pb.println(format!("{} finished {}", "[+]".green(), diff_meta.file.to_str().unwrap()));
                    pb.inc(1);
                    lint_messages
                },
                Err(_) => panic!("Unable to find file {:?}", diff_meta.file)
            }
        })
        .collect();

    debug!(logger, "Lint Messages = {:#?}", lint_messages);
    pb.finish_with_message("done");

    display::render(lint_messages);

    Ok(())
}
