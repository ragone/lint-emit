use std::io::Write;
use std::fs;
use dialoguer::{theme::ColorfulTheme, Checkboxes};
use failure::Error;
use serde::{Serialize, Deserialize};
use crate::lint::*;
use directories::ProjectDirs;

/// The config
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub linters: Option<Vec<LinterConfig>>
}

/// Get the config or create a new config in xdg path
pub fn get_config() -> Result<Config, Error> {
    // Determine if a config file exists, otherwise create it
    let proj_dirs = ProjectDirs::from("io", "ragone", "lint-emit").unwrap();
    let mut file_path = proj_dirs.config_dir().to_path_buf();
    file_path.push("config.toml");
    let config_path = if file_path.exists() {
        file_path
    } else {
        // Get the default config
        let default_config: Config = toml::from_str(include_str!("default_config.toml"))?;
        let linters = default_config.linters.unwrap();

        let selected_linters: Vec<LinterConfig> = match cfg!(windows) {
            true => {
                // Include all default linters
                linters.clone()
            },
            false => {
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

                linters
                    .clone()
                    .into_iter()
                    .filter(|linter| {
                        selected_names.contains(&linter.name.as_str())
                    })
                    .collect()
            }
        };

        let new_config = Config {
            linters: Some(selected_linters)
        };

        // Create config file from selection
        let mut config_file = fs::File::create(file_path.clone())?;
        write!(&mut config_file, "{}", toml::to_string(&new_config)?)?;
        println!("Successfully wrote configuration file to {:?}", file_path);

        file_path
    };

    // Get the config
    let config_string = fs::read_to_string(config_path).expect("Unable to read file");
    Ok(toml::from_str(&config_string)?)
}
