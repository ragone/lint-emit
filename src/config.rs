use std::io::Write;
use std::fs;
use dialoguer::{theme::ColorfulTheme, Checkboxes};
use failure::Error;
use serde::{Serialize, Deserialize};
use crate::lint::*;

/// The config
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub linters: Option<Vec<LinterConfig>>
}

/// Get the config or create a new config in xdg path
pub fn get_config() -> Result<Config, Error> {
    // Determine if a config file exists, otherwise create it
    let xdg_dirs = xdg::BaseDirectories::with_prefix("lint-emit").unwrap();
    let config_path = match xdg_dirs.find_config_file("config.toml") {
        Some(file_path) => file_path,
        None => {
            // Get the default config
            let default_config: Config = toml::from_str(include_str!("default_config.toml"))?;
            let linters = default_config.linters.unwrap();

            // Prompt user to select linters
            let linter_names: Vec<&str> = linters
                .iter()
                .map(|linter| linter.name.as_str())
                .collect();

            let selections = Checkboxes::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose linters [Press SPACE to select]")
                .items(&linter_names)
                .interact()
                .unwrap();

            let selected_names: Vec<&str> = selections
                .into_iter()
                .filter_map(|selection| linter_names.get(selection))
                .map(|selection| *selection)
                .collect();

            let selected_linters: Vec<LinterConfig> = linters
                .clone()
                .into_iter()
                .filter(|linter| {
                    selected_names.contains(&linter.name.as_str())
                })
                .collect();

            let new_config = Config {
                linters: Some(selected_linters)
            };

            // Create config file from selection
            let config_path = xdg_dirs.place_config_file("config.toml")
                                      .expect("Cannot create configuration directory");

            let mut config_file = fs::File::create(config_path.clone())?;
            write!(&mut config_file, "{}", toml::to_string(&new_config)?)?;
            println!("Successfully wrote configuration file to {:?}", config_path);

            config_path
        }
    };

    // Get the config
    let config_string = fs::read_to_string(config_path).expect("Unable to read file");
    Ok(toml::from_str(&config_string)?)
}
