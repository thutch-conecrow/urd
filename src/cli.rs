use clap::{Parser, Subcommand};

use crate::store::types::Sensitivity;

#[derive(Parser)]
#[command(name = "urd", about = "Configuration and secrets manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Set a config value (interactive if no arguments given)
    Set(SetArgs),
    /// Get a config value
    Get(GetArgs),
    /// List config values
    List(ListArgs),
    /// Remove a config item (values and metadata)
    Remove(RemoveArgs),
    /// Manage catalog metadata
    #[command(subcommand)]
    Catalog(CatalogCommand),
    /// Manage encryption keys
    #[command(subcommand)]
    Keys(KeysCommand),
    /// Assemble .env files from a topology
    Assemble(AssembleArgs),
    /// Validate items (completeness and consistency)
    Validate,
}

#[derive(Parser)]
pub struct AssembleArgs {
    /// Topology name from topologies.yaml
    #[arg(short, long)]
    pub topology: String,
    /// Assemble only this component
    #[arg(short, long)]
    pub component: Option<String>,
    /// Continue on missing store values (writes empty value instead of erroring)
    #[arg(long)]
    pub allow_missing: bool,
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
pub enum CatalogCommand {
    /// Add or update catalog metadata for an item
    Add(CatalogAddArgs),
    /// Remove an item entirely
    Remove(CatalogRemoveArgs),
    /// List items with their catalog metadata
    List(CatalogListArgs),
    /// Show full details of a catalog item
    Show(CatalogShowArgs),
}

#[derive(Parser)]
pub struct CatalogAddArgs {
    /// Config item ID
    pub id: String,
    /// Description of the config item
    #[arg(short, long)]
    pub description: Option<String>,
    /// Sensitivity level
    #[arg(short, long)]
    pub sensitivity: Option<Sensitivity>,
    /// Where to obtain this value
    #[arg(short, long)]
    pub origin: Option<String>,
    /// Expected environment(s)
    #[arg(short, long)]
    pub env: Vec<String>,
    /// Tag(s) (e.g., vendor:paddle)
    #[arg(short, long)]
    pub tag: Vec<String>,
}

#[derive(Parser)]
pub struct CatalogRemoveArgs {
    /// Config item ID to remove
    pub id: String,
}

#[derive(Parser)]
pub struct CatalogListArgs {
    /// Filter by environment
    #[arg(short, long)]
    pub env: Vec<String>,
    /// Filter by tag
    #[arg(short, long)]
    pub tag: Option<String>,
    /// Filter by sensitivity level
    #[arg(short, long)]
    pub sensitivity: Option<Sensitivity>,
}

#[derive(Parser)]
pub struct CatalogShowArgs {
    /// Config item ID
    pub id: String,
}

#[derive(Subcommand)]
pub enum KeysCommand {
    /// Generate a new encryption key for this directory
    Init,
    /// Show key status
    Status,
    /// Export the encryption key (for sharing with team members)
    Export,
}
