// Using yaml requires calling a clap macro `load_yaml!()` so we must use the '#[macro_use]'
// directive
#[macro_use]
extern crate clap;
use clap::App;
use std::process::Command;

fn main() {
    let yml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yml).get_matches();

    // Get commit range from args or default to HEAD
    let commit_range = matches.value_of("commit_range").unwrap_or("HEAD");

    println!("Commit range provided: {}", commit_range);


    println!("Changed files: {:?}", get_changed_files(commit_range));
}

fn get_changed_files(commit_range: &str) {
    Command::new("git")
        .arg("diff")
        .arg(commit_range)
        .arg("--name-only")
        .arg("--diff-filter=ACM")
        .output()
        .expect("Please ensure you have git installed");
}
