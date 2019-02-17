//! This tool aims to run multiple linters on a commit range compatible with `git`.
//!
//! Linters are great tools to enforce code style in your code, but it has some limitations: it can only lint entire files.
//! When working with legacy code, we often have to make changes to very large files (which would be too troublesome to fix all lint errors)
//! and thus it would be good to lint only the lines changed and not the entire file.
//!
//! `lint-emit` receives a commit range and uses the specified linters (defaults to `clippy`) to lint the changed files and filter only the errors introduced in the commit range (and nothing more).
//!
//! # Usage
//! ### Install
//! ```shell
//! $ cargo build --release
//! ```
//!
//! ### Lint the last commit
//! ```shell
//! $ lint-emit HEAD^..HEAD
//! ```
//!
//! # Examples
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
//!
//! # Compatible Linters
//! - Rust
//!   - `clippy`
//! - PHP
//!   - `phpmd`
//!   - `phpcs`

use regex::Regex;
use std::process::Command;
use std::path::PathBuf;
use slog::{trace};
use std::fs;
use failure::Error;
use failure::Fail;
use regex::NoExpand;
use serde::{Serialize, Deserialize};

/// Contains config of the linter
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinterConfig {
    pub name: String,
    pub cmd: String,
    pub args: Vec<String>,
    pub regex: String,
    pub ext: Vec<String>
}

/// Contains the line numbers which have changed for a given file
#[derive(Debug)]
pub struct DiffMeta {
    pub file: PathBuf,
    changed_lines: Vec<LineMeta>
}

/// Contains the changed lines and the snippets
#[derive(Debug)]
struct LineMeta {
    line: u32,
    source: String
}

/// Contains the lint message for a given file
#[derive(Debug)]
pub struct LintMessage {
    pub linter: String,
    pub file: PathBuf,
    pub line: u32,
    pub source: String,
    pub message: String
}

/// Return the output from running a linter on the whole project
pub fn get_lint_messages(linters: &Vec<&LinterConfig>, diff_meta: &DiffMeta, logger: &slog::Logger) -> Result<Vec<LintMessage>, Error> {
    let mut lint_messages: Vec<LintMessage> = vec![];
    for linter in linters.into_iter() {
        let re = Regex::new(&linter.regex)?;
        let output = get_lint_output(&linter, &diff_meta.file)?;
        trace!(logger, "Output = {:?}", output);
        for cap in re.captures_iter(&output) {
        trace!(logger, "Capture = {:#?}", cap);
            if let Some(lint_message) = get_lint_message(&linter, cap, diff_meta, logger) {
                trace!(logger, "Adding = {:#?}", lint_message);
                lint_messages.push(lint_message);
            }
        }
    }
    Ok(lint_messages)
}

/// Get the lint message
fn get_lint_message(linter: &LinterConfig, cap: regex::Captures, diff_meta: &DiffMeta, logger: &slog::Logger) -> Option<LintMessage> {
    let message = cap.name("message")?.as_str().to_owned();

    let file_name = match cap.name("file") {
        Some(file) => file.as_str(),
        None => diff_meta.file.to_str().unwrap()
    };

    let file = PathBuf::from(file_name);
    trace!(logger, "Processing file {:?}", file);

    let line = cap.name("line")?.as_str().parse::<u32>().unwrap();
    trace!(logger, "Processing line {:?}", line);

    let line_meta = diff_meta.changed_lines.iter().find(|x| x.line == line);

    // Filter here
    trace!(logger, "For {:?}", diff_meta);
    if diff_meta.file == file
        && line_meta.is_some() {
            return Some(LintMessage {
                linter: linter.name.to_string(),
                source: line_meta.unwrap().source.to_owned(),
                message,
                file,
                line
            })
        }
    None
}

/// Return the output from running a linter on the file
fn get_lint_output(linter: &LinterConfig, file: &PathBuf) -> Result<String, Error> {
    // Insert the file in the cmd
    let file_re = Regex::new(r"\{file\}")?;
    let args: Vec<String> = linter
        .args
        .iter()
        .map(|arg| file_re.replace(&arg, NoExpand(file.to_str().unwrap())).to_string())
        .collect();

    // Get the args split by whitespace
    let cmd_output = Command::new(&linter.cmd)
        .args(args)
        .output()?;

    // Figure where the output is
    let stdout = cmd_output.stdout;
    let stderr = cmd_output.stderr;

    let result = if stdout.is_empty() {
        stderr
    } else {
        stdout
    };

    Ok(String::from_utf8(result)?)
}

/// Return the line number for lines which have changed from `git diff`
fn get_changed_lines_from_diff(hunk: String) -> Result<Vec<LineMeta>, Error> {
    let mut line_number = 0;
    let re = Regex::new(r"\+([0-9]+)")?;
    let sanitize = Regex::new(r"^[-+ ]\s*")?;
    let changed_lines = hunk.lines().fold(vec![], |mut changed_lines, line| {
        if line.starts_with("@@") {
            // This is the line where the diff starts
            // So lets get the line number
            let start = re.find(&line).unwrap().as_str();
            line_number = start.parse().unwrap();
            line_number -= 1;
            return changed_lines;
        }

        if !line.starts_with('-') {
            // Increment the current line number if the line wasn't removed
            line_number += 1;
            if line.starts_with('+') {
                // Sanitize the line
                let source = sanitize.replace(line, "");

                // Add the line number of the line which was added
                changed_lines.push(LineMeta {
                    line: line_number,
                    source: source.to_string()
                });
                return changed_lines;
            }
        }
        changed_lines
    });
    Ok(changed_lines)
}

/// Returns the changed line numbers, split by file path
pub fn get_changed_lines(commit_range: &str, file: &PathBuf) -> Result<DiffMeta, Error> {
    let diff = get_diff(commit_range, &file)?;
    let changed_lines = get_changed_lines_from_diff(diff)?;
    let result = DiffMeta {
        file: file.to_path_buf(),
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

fn get_git_diff_output(commit_range: &str) -> Result<std::process::Output, Error> {
    let output = Command::new("git")
        .arg("diff")
        .arg(commit_range)
        .arg("--name-only")
        .arg("--diff-filter=ACM")
        .output()?;

    Ok(output)
}

/// Returns the changed files in a commit range using `git diff`
pub fn get_changed_files(commit_range: &str) -> Result<Vec<PathBuf>, Error> {
    let output = get_git_diff_output(commit_range)?;

    match output.status.success() {
        true => {
            let result = String::from_utf8(output.stdout)?
                .lines()
                .filter_map(|line| {
                    fs::canonicalize(line).ok()
                })
                .collect();
            Ok(result)
        },
        false => panic!("Git error")
    }
}

#[derive(Debug, Fail)]
pub enum LintError {
    #[fail(display = "IO error")]
    IO,
    #[fail(display = "Parsing error")]
    Parse
}
