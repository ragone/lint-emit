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
use std::process::{Command, Stdio};
use slog::{Level, Logger, Drain, info, debug, trace, o};
use slog_term::{TermDecorator, CompactFormat};
use failure::Error;
use lint_forge::*;
use std::path::PathBuf;
use indicatif::{ProgressBar};
use rayon::prelude::*;
use colored::*;
use itertools::*;

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

    // Get the linters from args or default to all linters
    let linters: Vec<String> = match matches.values_of("linters") {
        Some(linters_map) => linters_map.map(|linter| linter.to_owned()).collect(),
        None => linters_yml
            .as_hash()
            .unwrap()
            .keys()
            .filter_map(|linter| linter.clone().into_string())
            .collect()
    };
    debug!(logger, "Linters = {:#?}", linters);

    // Create LinterConfigs for linters
    let linter_configs: Vec<LinterConfig> = linters
        .par_iter()
        .map(|linter| {
            let config = linters_yml[linter.as_str()].clone();
            let cmd = config["cmd"].clone().into_string().unwrap();
            let regex = config["regex"].clone().into_string().unwrap();
            let ext: Vec<String> = config["ext"]
                .clone()
                .into_iter()
                .filter_map(|ext| ext.into_string())
                .collect();
            let args: Vec<String> = config["args"]
                .clone()
                .into_iter()
                .filter_map(|arg| arg.into_string())
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
                    pb.println(format!("{} .{} files with: {}", "Processing".blue(), ext.bold(), linter_str.bold()));
                    diff_metas
                        .collect::<Vec<&DiffMeta>>()
                        .par_iter()
                        .flat_map(|diff_meta| {
                            let lint_messages = get_lint_messages_for_file(&diff_meta, &valid_linters, &logger);
                            pb.println(format!("{} {}", "Finished".green(), diff_meta.file.to_str().unwrap()));
                            pb.inc(1);
                            lint_messages
                        })
                        .collect::<Vec<LintMessage>>()
                },
                true => {
                    pb.println(format!("{} No linters found for .{}", "Skipping".yellow(), ext.bold()));
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
