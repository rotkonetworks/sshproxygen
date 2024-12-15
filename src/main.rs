use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Operation requires root privileges")]
    NotRoot,
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML deserialize error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Proxy {
    target: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    ssh_key: PathBuf,
    proxies: HashMap<String, Proxy>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ssh_key: PathBuf::from("/etc/sshproxygen/id_rsa"),
            proxies: HashMap::new(),
        }
    }
}

#[derive(Parser)]
#[command(author, version, about = "SSH proxy user management tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short = 'i', long = "identity", help = "SSH identity file")]
    identity: Option<PathBuf>,

    #[arg(short = 'c', long = "config", default_value = "/etc/sshproxygen/config.toml")]
    config: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Add new proxy user
    Add {
        /// Format: bkk10:proxyssh@172.16.10.1
        proxy_string: String,
    },
    /// Remove proxy user
    Remove {
        proxy_user: String,
    },
    /// List all proxies
    List,
    /// Install proxies from config
    Install,
}

impl Config {
    fn load(path: &Path) -> Result<Self, Error> {
        if !path.exists() {
            return Ok(Config::default());
        }
        Ok(toml::from_str(&fs::read_to_string(path)?)?)
    }

    fn save(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(fs::write(path, toml::to_string_pretty(self)?)?)
    }
}

fn parse_proxy_string(s: &str) -> Result<(String, String, String), Error> {
    let parts: Vec<&str> = s.split([':', '@']).collect();
    if parts.len() != 3 {
        return Err(Error::Config("Invalid proxy string format".into()));
    }
    Ok((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
}

fn create_user(username: &str) -> Result<(), Error> {
    let status = Command::new("useradd")
        .args([
            "--system",
            "--shell", "/bin/false",
            "--no-create-home",
            username
        ])
        .status()?;

    if !status.success() {
        return Err(Error::Config(format!("Failed to create user {}", username)));
    }
    Ok(())
}

fn update_sshd_config(proxy_user: &str, target_user: &str, target: &str, key_path: &Path) -> Result<(), Error> {
    let config_entry = format!(
        "\nMatch User {}\n  ForceCommand ssh -i {} -W {}:22 {}@{}\n",
        proxy_user, key_path.display(), target, target_user, target
    );

    OpenOptions::new()
        .append(true)
        .open("/etc/ssh/sshd_config")?
        .write_all(config_entry.as_bytes())?;

    Command::new("systemctl")
        .args(["restart", "sshd"])
        .status()?;

    Ok(())
}

fn ensure_root() -> Result<(), Error> {
    if !nix::unistd::geteuid().is_root() {
        return Err(Error::NotRoot);
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    ensure_root()?;

    let cli = Cli::parse();
    let mut config = Config::load(&cli.config)?;

    if let Some(identity) = cli.identity {
        config.ssh_key = identity;
    }

    match cli.command {
        Commands::Add { proxy_string } => {
            let (proxy_user, target_user, target) = parse_proxy_string(&proxy_string)?;
            
            create_user(&proxy_user)?;
            update_sshd_config(&proxy_user, &target_user, &target, &config.ssh_key)?;
            
            config.proxies.insert(proxy_user.clone(), Proxy {
                target,
                port: 22,
            });
            config.save(&cli.config)?;
        }

        Commands::Remove { proxy_user } => {
            if config.proxies.remove(&proxy_user).is_some() {
                Command::new("userdel").arg(&proxy_user).status()?;
                config.save(&cli.config)?;
            }
        }

        Commands::List => {
            println!("SSH key: {}", config.ssh_key.display());
            println!("\nConfigured proxies:");
            for (proxy_user, proxy) in &config.proxies {
                println!("{} -> {}", proxy_user, proxy.target);
            }
        }

        Commands::Install => {
            for (proxy_user, proxy) in &config.proxies {
                create_user(proxy_user)?;
                update_sshd_config(
                    proxy_user,
                    "proxyssh",
                    &proxy.target,
                    &config.ssh_key
                )?;
            }
        }
    }

    Ok(())
}
