extern crate clap;
extern crate slog;
extern crate slog_term;
extern crate slog_async;
extern crate lint_emit;
extern crate itertools;
extern crate walkdir;
extern crate tui;
extern crate termion;
extern crate serde_yaml;

mod display;

use clap::{Arg, App};
use std::process::{Command, Stdio};
use slog::{Level, Logger, Drain, info, debug, trace, o};
use slog_term::{TermDecorator, CompactFormat};
use failure::Error;
use lint_emit::*;
use indicatif::{ProgressBar};
use rayon::prelude::*;
use colored::*;
use itertools::*;
use std::fs::File;

fn main() -> Result<(), Error> {
    // Get the config
    let file = File::open("/home/ragone/Developer/lint-emit/src/linters.yml")?;
    let linters_yml: serde_yaml::Value = serde_yaml::from_reader(file)?;
    let linters_map = linters_yml.as_mapping().unwrap();
    let possible_values: Vec<&str> = linters_map.iter().map(|(key, _)| key.as_str().unwrap()).collect();
    let matches = App::new("lint-emit")
        .version("1.0")
        .author("Alex Ragone <ragonedk@gmail.com>")
        .about("Lint your git diffs!")
        .arg(Arg::with_name("COMMIT_RANGE")
             .short("c")
             .long("config")
             .default_value("HEAD")
             .help("Commit range provided to diff")
             .index(1))
        .arg(Arg::with_name("LINTERS")
             .short("l")
             .long("linters")
             .help("The linters to use")
             .possible_values(&possible_values)
             .takes_value(true)
             .multiple(true))
        .arg(Arg::with_name("CONFIG")
             .short("c")
             .long("config")
             .help("The YAML config file location"))
        .arg(Arg::with_name("VERBOSE")
             .short("v")
             .long("verbose")
             .help("Control the output verbosity")
             .multiple(true))
        .get_matches();

    // Setup logging level
    let min_log_level = match matches.occurrences_of("VERBOSE") {
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
    let commit_range = matches.value_of("COMMIT_RANGE").unwrap();
    debug!(logger, "Commit Range = {:#?}", commit_range);

    // Get the linters from args or default to all linters
    let linters: Vec<String> = match matches.values_of("LINTERS") {
        Some(linters_map) => linters_map.map(|linter| linter.to_owned()).collect(),
        None => possible_values.into_iter().map(|key| key.to_owned()).collect()
    };
    debug!(logger, "Linters = {:#?}", linters);

    // Create LinterConfigs for linters
    let linter_configs: Vec<LinterConfig> = linters
        .par_iter()
        .map(|linter| {
            let config = &linters_yml[linter.as_str()];
            let cmd = config["cmd"].as_str().unwrap().to_owned();
            let regex = config["regex"].as_str().unwrap().to_owned();
            let ext: Vec<String> = config["ext"]
                .as_sequence()
                .unwrap()
                .into_iter()
                .map(|ext| ext.as_str().unwrap().to_owned())
                .collect();
            let args: Vec<String> = config["args"]
                .as_sequence()
                .unwrap()
                .into_iter()
                .map(|arg| arg.as_str().unwrap().to_owned())
                .collect();
            LinterConfig {
                name: linter.to_owned(),
                cmd,
                args,
                regex,
                ext
            }
        })
        .filter(|linter_config| {
            // Check to see if the file executable
            Command::new(&linter_config.cmd)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok()
        })
        .collect();
    debug!(logger, "Linter Configs = {:#?}", linter_configs);

    run(commit_range, linter_configs, logger)
}

/// Run the linters across the whole project and return the linting messages
/// for just the changed lines
fn run(commit_range: &str, linters: Vec<LinterConfig>, logger: slog::Logger) -> Result<(), Error> {

    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(200);

    // Get the changed files
    spinner.set_message("Getting changes files");
    let changed_files = get_changed_files(commit_range)?;
    debug!(logger, "Changed Files = {:#?}", changed_files);

    // Get the changed files and line numbers
    spinner.set_message("Getting changed lines");
    let diff_metas: Vec<DiffMeta> = changed_files
        .par_iter()
        .map(|file| get_changed_lines(commit_range, &file).unwrap())
        .collect();
    trace!(logger, "Diff Metas = {:#?}", diff_metas);
    spinner.finish_and_clear();

    let pb = ProgressBar::new(diff_metas.len() as u64);

    // Get the output from running the linters for each file
    let lint_messages: Vec<LintMessage> = diff_metas
        .iter()
        .sorted_by_key(|diff_meta| {
            match diff_meta.file.extension() {
                Some(ext) => ext.to_str().unwrap(),
                None => "None"
            }
        })
        .group_by(|diff_meta| {
            match &diff_meta.file.extension() {
                Some(ext) => ext.to_str().unwrap(),
                None => "None"
            }
        })
        .into_iter()
        .flat_map(|(ext, diff_metas)| {
            let valid_linters: Vec<&LinterConfig> = get_valid_linters(ext, &linters);
            match valid_linters.is_empty() {
                false => {
                    let linter_str = valid_linters.iter().map(|linter| &linter.name).join(", ");
                    pb.println(format!("{} [.{}] Linters: {}", "Processing".blue(), ext.bold(), linter_str.bold()));
                    diff_metas
                        .collect::<Vec<&DiffMeta>>()
                        .par_iter()
                        .flat_map(|diff_meta| {
                            let lint_messages = get_lint_messages_for_file(&diff_meta, &valid_linters, &logger);
                            pb.println(format!("{} {}", "✓".green(), diff_meta.file.to_str().unwrap().dimmed()));
                            pb.inc(1);
                            lint_messages
                        })
                        .collect::<Vec<LintMessage>>()
                },
                true => {
                    pb.println(format!("{}   [.{}] No linters found", "Skipping".yellow(), ext.bold()));
                    vec![]
                }
            }
        })
        .collect();

    trace!(logger, "Lint Messages = {:#?}", lint_messages);
    pb.finish_and_clear();

    // Output the result
    display::render(lint_messages);

    Ok(())
}

fn get_lint_messages_for_file(diff_meta: &DiffMeta, linters: &Vec<&LinterConfig>, logger: &slog::Logger) -> Vec<LintMessage> {
    let lint_messages = get_lint_messages(linters, &diff_meta, &logger);
    match lint_messages {
        Ok(lint_messages) => lint_messages,
        Err(_) => panic!("Unable to find file {:?}", diff_meta.file)
    }
}

fn get_valid_linters<'a>(ext: &str, linters: &'a Vec<LinterConfig>) -> Vec<&'a LinterConfig> {
    linters
        .iter()
        .filter(|linter| {
            linter.ext.contains(&ext.to_owned())
        })
        .collect()
}
