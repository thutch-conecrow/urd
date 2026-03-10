use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "urd", about = "Configuration and secrets manager")]
pub struct Cli {
    /// Operate on the global store (~/.config/urd/) instead of the local project store
    #[arg(short, long)]
    pub global: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Set a config value (interactive if no arguments given)
    Set(SetArgs),
    /// Get a config value
    Get(GetArgs),
    /// List config items
    List(ListArgs),
    /// Remove a config item
    Remove(RemoveArgs),
    /// Manage encryption keys
    #[command(subcommand)]
    Keys(KeysCommand),
    /// Validate catalog against store
    Validate,
}

#[derive(Parser)]
pub struct SetArgs {
    /// Config item ID (e.g., `paddle.api_key`)
    pub id: Option<String>,
    /// Environment(s) (e.g., --env dev --env prod)
    #[arg(short, long)]
    pub env: Vec<String>,
    /// Value to set
    pub value: Option<String>,
    /// Encrypt as sensitive
    #[arg(long, conflicts_with = "secret")]
    pub sensitive: bool,
    /// Encrypt as secret
    #[arg(long)]
    pub secret: bool,
}

#[derive(Parser)]
pub struct GetArgs {
    /// Config item ID
    pub id: String,
    /// Environment(s) — omit to show all
    #[arg(short, long)]
    pub env: Vec<String>,
    /// Reveal encrypted values as plaintext
    #[arg(short, long)]
    pub reveal: bool,
}

#[derive(Parser)]
pub struct ListArgs {
    /// Filter by tag (e.g., vendor:paddle)
    #[arg(short, long)]
    pub tag: Option<String>,
    /// Filter by environment(s)
    #[arg(short, long)]
    pub env: Vec<String>,
    /// Reveal encrypted values as plaintext
    #[arg(short, long)]
    pub reveal: bool,
}

#[derive(Parser)]
pub struct RemoveArgs {
    /// Config item ID to remove
    pub id: String,
}

#[derive(Subcommand)]
pub enum KeysCommand {
    /// Generate a new age keypair
    Init,
    /// Show key status and public key
    Status,
    /// Export the public key
    Export,
}
