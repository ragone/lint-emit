//! This tool aims to run multiple linters on a commit range compatible with `git`.
//!
//! Linters are great tools to enforce code style in your code, but it has some limitations: it can only lint entire files.
//! When working with legacy code, we often have to make changes to very large files (which would be too troublesome to fix all lint errors)
//! and thus it would be good to lint only the lines changed and not the entire file.
//!
//! `lint-forge` receives a commit range and uses the specified linters (defaults to `clippy`) to lint the changed files and filter only the errors introduced in the commit range (and nothing more).
//!
//! # Usage
//! ### Install
//! ```shell
//! $ cargo build --release
//! ```
//!
//! ### Lint the last commit
//! ```shell
//! $ lint-forge HEAD^..HEAD
//! ```
//!
//! # Examples
//! ### Lint the last 3 commits
//! ```shell
//! $ lint-forge HEAD~3..HEAD
//! ```
//!
//! ### Lint local changes that are not yet committed
//! ```shell
//! $ lint-forge HEAD
//! # or
//! $ lint-forge
//! ```
//!
//! ### Lint using `phpmd` and `phpcs`
//! ```shell
//! $ lint-forge --linters phpmd phpcs
//! ```
//!
//! # Compatible Linters
//! - Rust
//!   - `clippy`
//! - PHP
//!   - `phpmd`
//!   - `phpcs`

mod display;

use regex::Regex;
use std::process::Command;
use std::path::PathBuf;
use slog::{debug, trace};
use std::fs;

/// Contains the line numbers which have changed for a given file
#[derive(Debug)]
struct DiffMeta {
    file: PathBuf,
    changed_lines: Vec<u32>
}

/// Contains the lint message for a given file
#[derive(Debug)]
pub struct LintMessage {
    linter: String,
    file: PathBuf,
    line: u32,
    message: String
}

/// Run the linters across the whole project and return the linting messages
/// for just the changed lines
pub fn run(commit_range: &str, linters: Vec<&str>, logger: slog::Logger) -> Result<(), Error> {
    // Get the changed files
    let changed_files = get_changed_files(commit_range)?;
    debug!(logger, "Changed Files = {:#?}", changed_files);

    // Get the changed files and line numbers
    let diff_metas: Vec<DiffMeta> = changed_files
        .into_iter()
        .map(|file| get_changed_lines(commit_range, file).unwrap())
        .collect();


    // Get the output from running the linters for each file
    let lint_messages: Vec<LintMessage> = diff_metas
        .into_iter()
        .flat_map(|diff_meta| {
            get_lint_messages(&linters, &diff_meta, &logger).unwrap()
        })
        .collect();

    display::render(lint_messages);

    Ok(())
}

/// Return the output from running a linter on the whole project
fn get_lint_messages(linters: &Vec<&str>, diff_meta: &DiffMeta, logger: &slog::Logger) -> Result<Vec<LintMessage>, Error> {
    let mut lint_messages: Vec<LintMessage> = vec![];
    for linter in linters.into_iter() {
        let regex = match linter {
            &"clippy" => r"(?P<message>.*)\n.*--> (?P<file>.*):(?P<line>\d*):",
            &"phpmd" => r"(?P<file>.*):(?P<line>\d*)\s*(?P<message>.*)",
            &"phpcs" => r"(?P<file>.*):(?P<line>\d*):.*: (?P<message>.*)",
            _ => r""
        };

        let re = Regex::new(regex).unwrap();
        let output = get_lint_output(linter, &diff_meta.file).unwrap();
        trace!(logger, "Output = {:?}", output);
        for cap in re.captures_iter(&output) {
        trace!(logger, "Capture = {:#?}", cap);
            if let Some(lint_message) = get_lint_messages_regex(linter, cap, diff_meta, logger) {
                lint_messages.push(lint_message);
            }
        }
    }
    Ok(lint_messages)
}

/// Get the lint messages
fn get_lint_messages_regex(linter: &str, cap: regex::Captures, diff_meta: &DiffMeta, logger: &slog::Logger) -> Option<LintMessage> {
    let message = cap.name("message").unwrap().as_str().to_owned();

    let file = PathBuf::from(cap.name("file").unwrap().as_str());
    trace!(logger, "Processing file {:?}", file);

    let line = cap.name("line").unwrap().as_str().parse::<u32>().unwrap();
    trace!(logger, "Processing line {:?}", line);

    // Filter here
    trace!(logger, "For {:?}", diff_meta);
    if diff_meta.file == file
        && diff_meta.changed_lines.contains(&line) {
            return Some(LintMessage {
                linter: linter.to_owned(),
                message,
                file,
                line
            })
        }
    None
}

/// Return the output from running a linter on the file
fn get_lint_output(linter: &str, file: &PathBuf) -> Result<String, Error> {
    let output = match linter {
        "clippy" => Command::new("cargo").arg("check").output()?.stderr,
        "phpmd" => Command::new("phpmd").arg(file.to_str().unwrap()).arg("text").arg("cleancode,codesize,controversial,design,naming,unusedcode").output()?.stdout,
        "phpcs" => Command::new("phpcs").arg(file.to_str().unwrap()).arg("--report=emacs").output()?.stdout,
        _ => Command::new("cargo").arg("clippy").output()?.stderr
    };
    Ok(String::from_utf8(output)?)
}

/// Return the line number for lines which have changed from `git diff`
fn get_changed_lines_from_diff(hunk: String) -> Vec<u32> {
    let mut line_number = 0;
    hunk.lines().fold(vec![], |mut changed_lines, line| {
        if line.starts_with("@@") {
            // This is the line where the diff starts
            // So lets get the line number
            let re = Regex::new(r"\+([0-9]+)").unwrap();
            let start = re.find(&line).unwrap().as_str();
            line_number = start.parse().unwrap();
            line_number -= 1;
            return changed_lines;
        }

        if !line.starts_with('-') {
            // Increment the current line number if the line wasn't removed
            line_number += 1;

            if line.starts_with('+') {
                // Add the line number of the line which was added
                changed_lines.push(line_number);
                return changed_lines;
            }
        }

        changed_lines
    })
}

/// Returns the changed line numbers, split by file path
fn get_changed_lines(commit_range: &str, file: PathBuf) -> Result<DiffMeta, Error> {
    let diff = get_diff(commit_range, &file)?;
    let changed_lines = get_changed_lines_from_diff(diff);
    let result = DiffMeta {
        file,
        changed_lines
    };

    Ok(result)
}

/// Return the output of `git diff`
fn get_diff(commit_range: &str, file: &PathBuf) -> Result<String, Error> {
    let output = Command::new("git")
        .arg("diff")
        .arg(commit_range)
        .arg(file)
        .output()?;

    Ok(String::from_utf8(output.stdout)?)
}

/// Returns the changed files in a commit range using `git diff`
fn get_changed_files(commit_range: &str) -> Result<Vec<PathBuf>, Error> {
    let output = Command::new("git")
        .arg("diff")
        .arg(commit_range)
        .arg("--name-only")
        .arg("--diff-filter=ACM")
        .output()?;


    let result = String::from_utf8(output.stdout)?
        .lines()
        .map(|line| {
            fs::canonicalize(line).unwrap()
        })
        .collect();

    Ok(result)
}

#[derive(Debug)]
pub enum Error {
    IO,
    Parse,
    Regex,
    Slog,
    None
}

impl std::convert::From<std::io::Error> for Error {
    fn from(_err: std::io::Error) -> Error {
        Error::IO
    }
}

impl std::convert::From<std::string::FromUtf8Error> for Error {
    fn from(_err: std::string::FromUtf8Error) -> Error {
        Error::Parse
    }
}

impl std::convert::From<regex::Error> for Error {
    fn from(_err: regex::Error) -> Error {
        Error::Regex
    }
}

impl std::convert::From<slog::Error> for Error {
    fn from(_err: slog::Error) -> Error {
        Error::Slog
    }
}
