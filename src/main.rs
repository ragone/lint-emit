// Using yaml requires calling a clap macro `load_yaml!()` so we must use the '#[macro_use]'
// directive
#[macro_use]
extern crate clap;
extern crate regex;

use regex::Regex;
use clap::App;
use std::process::Command;
use std::path::PathBuf;

fn main() {
    let yml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yml).get_matches();

    // Get commit range from args or default to HEAD
    let commit_range = matches.value_of("commit_range").unwrap_or("HEAD");

    println!("Commit range provided: {}", commit_range);

    let changed_files = get_changed_files(commit_range).unwrap();
    let changed_file_line_map: Vec<Vec<String>> = changed_files
        .iter()
        .map(|file| {
            get_changed_file_line_map(commit_range, &file).unwrap()
        })
        .collect();

    dbg!(changed_file_line_map);
}

/// Return the lines which have changed from `git diff`
fn get_changed_lines_from_diff(hunk: String) -> Vec<String> {
    let mut line_number = 0;

    hunk.lines().fold(vec![], |mut changed_lines, line| {
        if line.starts_with("@@") {
            let re = Regex::new(r"\+([0-9]+)").unwrap();
            let matches = re.is_match(&line);
            dbg!(matches);
        }

        if !line.starts_with("-") {
            line_number += 1;

            if line.starts_with("+") {
                changed_lines.push(line_number.to_string());
            }
        }

        changed_lines
    })
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

fn get_changed_file_line_map(commit_range: &str, file: &PathBuf) -> Result<Vec<String>, Error> {
    let diff = get_diff(commit_range, file)?;
    let changed_lines = get_changed_lines_from_diff(diff);

    Ok(changed_lines)
}

/// Get the changed files in a commit range using `git diff`
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
            PathBuf::from(line)
        })
        .collect();

    Ok(result)
}

#[derive(Debug)]
enum Error {
    IOError,
    ParseError
}

impl std::convert::From<std::io::Error> for Error {
    fn from(_err: std::io::Error) -> Error {
        Error::IOError
    }
}

impl std::convert::From<std::string::FromUtf8Error> for Error {
    fn from(_err: std::string::FromUtf8Error) -> Error {
        Error::ParseError
    }
}
