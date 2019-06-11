mod channel {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct Target {
        pub available: bool,
    }

    #[derive(Deserialize)]
    pub struct Rust {
        pub version: String,
        pub target: indexmap::IndexMap<String, Target>,
    }

    #[derive(Deserialize)]
    pub struct Packages {
        pub rust: Rust,
    }

    #[derive(Deserialize)]
    pub struct Channel {
        pub pkg: Packages,
    }
}

mod tiers {
    use serde::{Serialize, Deserialize};
    use indexmap::IndexMap;

    #[derive(Serialize, Deserialize)]
    pub struct Platform {
        pub tuple: String,
        pub std: String,
        pub rustc: Option<String>,
        pub cargo: Option<String>,
        pub notes: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Tier {
        pub description: String,
        pub platforms: Vec<Platform>,
        pub footnotes: String,
    }

    /// `tiers.yaml` content.
    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Tiers {
        /// Maps Tier Name -> Data
        pub tiers: IndexMap<String, Tier>,
    }
}

mod config {
    use super::tiers::Tiers;
    use serde::Serialize;

    #[derive(Serialize)]
    pub struct Channel {
        pub vers: String,
        pub platforms: Vec<String>,
    }

    /// The Jekyll `_config.yaml` data.
    #[derive(Serialize)]
    pub struct Config {
        pub exclude: &'static [&'static str],
        pub rustup: Vec<String>,
        pub channels: indexmap::IndexMap<&'static str, Channel>,
        pub tiers: Tiers,
    }
}

use indexmap::IndexMap;
use regex::Regex;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};

const RUSTUP_URLS: &str =
    "https://raw.githubusercontent.com/rust-lang/rustup.rs/stable/ci/cloudfront-invalidation.txt";

const CHANNELS: &[&str] = &["stable", "beta", "nightly"];
const CHANNEL_URL_PREFIX: &str = "https://static.rust-lang.org/dist/channel-rust-";

fn main() -> Result<(), Box<dyn Error>> {
    let tiers: tiers::Tiers = serde_yaml::from_reader(File::open("tiers.yaml")?)?;

    let mut cfg = config::Config {
        exclude: &["target", "vendor"],
        rustup: Vec::new(),
        channels: IndexMap::with_capacity(CHANNELS.len()),
        tiers,
    };

    let rustup_url_regex = Regex::new(r"^rustup/dist/([^/]+)/rustup-init(?:\.exe)?$").unwrap();
    for line in BufReader::new(reqwest::get(RUSTUP_URLS)?).lines() {
        if let Some(m) = rustup_url_regex.captures(&(line?)) {
            cfg.rustup.push(m.get(1).unwrap().as_str().to_string());
        }
    }
    eprintln!("Found {} targets for rustup", cfg.rustup.len());

    for channel_name in CHANNELS {
        let channel_url = format!("{}{}.toml", CHANNEL_URL_PREFIX, channel_name);
        let content = reqwest::get(&channel_url)?.text()?;
        let rust = toml::from_str::<channel::Channel>(&content)?.pkg.rust;
        eprintln!(
            "Found {} targets for {} channel (v{})",
            rust.target.len(),
            channel_name,
            rust.version
        );

        let vers = rust.version.split(' ').next().unwrap().to_string();
        let platforms = rust
            .target
            .into_iter()
            .filter_map(|(target, content)| {
                if content.available {
                    Some(target)
                } else {
                    None
                }
            })
            .collect();

        cfg.channels
            .insert(channel_name, config::Channel { vers, platforms });
    }

    let config_file = File::create("_config.yml")?;
    serde_yaml::to_writer(config_file, &cfg)?;

    Ok(())
}
