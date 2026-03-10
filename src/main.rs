mod catalog;
mod cli;
mod crypto;
mod paths;
mod store;

use clap::Parser;
use cli::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Set(args) => store::commands::set(args, cli.global)?,
        Command::Get(ref args) => store::commands::get(args, cli.global)?,
        Command::List(ref args) => store::commands::list(args, cli.global)?,
        Command::Remove(ref args) => store::commands::remove(args, cli.global)?,
        Command::Keys(ref cmd) => crypto::commands::keys(cmd)?,
        Command::Validate => catalog::commands::validate()?,
    }

    Ok(())
}
