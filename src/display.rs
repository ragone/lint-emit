use super::LintMessage;
use itertools::Itertools;
use colored::*;
use walkdir::WalkDir;
use std::path::PathBuf;

pub fn render(lint_messages: Vec<LintMessage>) {
    if lint_messages.len() == 0 {
        println!("{}", "No errors found! Nice.".green().bold());
    }
    // Group the ouput by file
    for (file, outputs) in &lint_messages.into_iter().group_by(|elt| elt.file.to_owned()) {
        let project_root = get_project_root(&file);
        let file_name = file.strip_prefix(&project_root).unwrap().to_str().unwrap();
        outputs
            .group_by(|lint_message| lint_message.line)
            .into_iter()
            .for_each(|(line, lint_messages)| {
                println!("{}:{}", file_name.green(), line.to_string().dimmed());
                print_lint_message(lint_messages.collect(), line);
            });
    }
}

/// Print the lint message to stdout
fn print_lint_message(lint_messages: Vec<LintMessage>, line: u32) {
    let line_string = line.to_string();
    let padding = &str::repeat(" ", line_string.len());
    let source = &lint_messages.first().unwrap().source;
    let vertical_line = format!("{} {}", padding, "|".blue());
    println!("{}", vertical_line);
    println!("{} {} {}", line_string.blue(), "|".blue(), source);
    println!("{}", vertical_line);

    lint_messages
        .into_iter()
        .group_by(|lint_message| lint_message.linter.to_owned())
        .into_iter()
        .for_each(|(linter, lint_messages)| {
            println!("[{}]",linter.blue().bold());
            lint_messages
                .into_iter()
                .for_each(|lint_message| {
                    let message = lint_message.message;
                    let linter = lint_message.linter;
                    println!("{} {}", "-->".blue(), message.bold());
                });
        });
    println!("");
}


/// Recrsively looks for a parent directory containing .git and returns the path
fn get_project_root(file: &PathBuf) -> PathBuf {
    for entry in WalkDir::new(&file)
        .into_iter()
        .filter_map(|e| e.ok()) {
            let file_name = entry.file_name().to_string_lossy();
            if file_name == ".git" {
                return entry.path().parent().unwrap().to_path_buf()
            }
        }
    let parent_file = file.parent().to_owned().unwrap().to_path_buf();
    get_project_root(&parent_file)
}
