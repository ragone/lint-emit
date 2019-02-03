use super::LintMessage;
use itertools::Itertools;
use colored::*;
use walkdir::WalkDir;
use std::path::PathBuf;

pub fn render(lint_messages: Vec<LintMessage>) {
    // Group the ouput by file
    for (file, outputs) in &lint_messages.into_iter().group_by(|elt| elt.file.to_owned()) {
        let project_root = get_project_root(file);

        for lint_message in outputs.sorted_by_key(|line_message| line_message.line) {
            print_lint_message(lint_message, &project_root);
        }
    }
}

/// Print the lint message to stdout
fn print_lint_message(lint_message: LintMessage, project_root: &PathBuf) {
    let file_name = lint_message.file.strip_prefix(&project_root).unwrap().to_str().unwrap();
    let line_number = lint_message.line.to_string();
    let message = lint_message.message;
    println!("{}:{}", file_name.green().bold(), line_number.dimmed());
    println!("{}", message);
}


/// Recrsively looks for a parent directory containing .git and returns the path
fn get_project_root(file: PathBuf) -> PathBuf {
    for entry in WalkDir::new(&file)
        .into_iter()
        .filter_map(|e| e.ok()) {
            let file_name = entry.file_name().to_string_lossy();
            if file_name == ".git" {
                return entry.path().parent().unwrap().to_path_buf()
            }
        }
    let parent_file = file.parent().to_owned().unwrap().to_path_buf();
    get_project_root(parent_file)
}
