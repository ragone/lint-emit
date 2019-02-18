//! This tool aims to run **multiple** linters on a commit range compatible with `git`.
//!
//! Inspired by [lint-diff](https://github.com/grvcoelho/lint-diff) and [lint-staged](https://github.com/okonet/lint-staged)
//! > Linters are great tools to enforce code style in your code, but it has some limitations: it can only lint entire files.
//! > When working with legacy code, we often have to make changes to very large files (which would be too troublesome to fix all lint errors)
//! > and thus it would be good to lint only the lines changed and not the entire file.
//!
//! > `lint-emit` receives a commit range and uses the specified linters to lint the changed files and filter only the errors introduced in the commit range (and nothing more).
//!
//! # Configuration
//! You can add a linter by editing the config file found in your xdg path.
//! If no config file is found, you will be asked which default linters you would like to add.
//! ```toml
//! [[linters]]
//! name = "eslint"
//! cmd = "eslint"
//! args = ["{file}", "-f=compact"]
//! regex = '(?P<file>.*): line (?P<line>\d*), col \d*, (?P<message>.*)'
//! ext = ["js", "jsx"]
//! ```
//!
//! # Usage
//!
//! ### Lint the last commit
//! ```shell
//! $ lint-emit HEAD^..HEAD
//! ```
//!
//! ### Lint the last 3 commits
//! ```shell
//! $ lint-emit HEAD~3..HEAD
//! ```
//!
//! ### Lint local changes that are not yet committed
//! ```shell
//! $ lint-emit HEAD
//! # or
//! $ lint-emit
//! ```
//!
//! ### Lint using `phpmd` and `phpcs`
//! ```shell
//! $ lint-emit --linters phpmd phpcs
//! ```

extern crate clap;
extern crate slog;
extern crate slog_term;
extern crate slog_async;
extern crate itertools;
extern crate walkdir;
extern crate serde;
extern crate dialoguer;
extern crate xdg;
extern crate toml;

pub mod config;
mod lint;
mod display;
mod logger;

use clap::{Arg, App, AppSettings};
use std::process::{Command, Stdio};
use slog::{debug, trace};
use failure::Error;
use indicatif::{ProgressBar};
use rayon::prelude::*;
use colored::*;
use itertools::*;
use lint::*;
use config::*;
use logger::*;

fn main() -> Result<(), Error> {
    let config = get_config()?;
    let linters = config.linters.unwrap();
    let possible_values: Vec<&str> = linters.iter().map(|linter| linter.name.as_str()).collect();
    let matches = App::new("lint-emit")
        .version("0.3")
        .author("Alex Ragone <ragonedk@gmail.com>")
        .about("Lint your git diffs!")
        .setting(AppSettings::ColoredHelp)
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
        .arg(Arg::with_name("VERBOSE")
             .short("v")
             .long("verbose")
             .help("Control the output verbosity")
             .multiple(true))
        .get_matches();

    let verbosity = matches.occurrences_of("VERBOSE");
    let logger = setup_logger(verbosity);

    // Get commit range from args or default to HEAD
    let commit_range = matches.value_of("COMMIT_RANGE").unwrap();
    debug!(logger, "Commit Range = {:#?}", commit_range);

    // Get the linters from args or default to all linters
    let linter_args: Vec<String> = matches.values_of("LINTERS")
                                          .unwrap_or_default()
                                          .map(|linter| linter.to_owned())
                                          .collect();

    debug!(logger, "Linters = {:#?}", linters);

    // Create LinterConfigs for linters
    let linter_configs: Vec<LinterConfig> = linters
        .into_iter()
        .filter(|linter_config| {
            if !linter_args.is_empty() {
                linter_args.contains(&linter_config.name)
            } else {
                true
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
