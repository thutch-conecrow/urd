mod assembly;
mod catalog;
mod cli;
mod config;
mod crypto;
mod import;
mod init;
mod paths;
mod store;
mod tui;

use clap::Parser;
use cli::{CatalogCommand, Cli, Command, ConfigCommand};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        return tui::run();
    };

    match command {
        Command::Set(args) => store::commands::set(args)?,
        Command::Get(ref args) => store::commands::get(args)?,
        Command::List(ref args) => store::commands::list(args)?,
        Command::Remove(ref args) => store::commands::remove(args)?,
        Command::Catalog(ref cmd) => match cmd {
            CatalogCommand::Add(args) => catalog::commands::add(args)?,
            CatalogCommand::Remove(args) => catalog::commands::remove(&args.id)?,
            CatalogCommand::List(args) => catalog::commands::list(args)?,
            CatalogCommand::Show(args) => catalog::commands::show(args)?,
        },
        Command::Assemble(ref args) => assembly::commands::assemble(args)?,
        Command::Keys(ref cmd) => crypto::commands::keys(cmd)?,
        Command::Validate => catalog::commands::validate()?,
        Command::Import(ref args) => import::commands::import(args)?,
        Command::Config(ref cmd) => match cmd {
            ConfigCommand::SetDefaults(args) => config::commands::set_defaults(args)?,
            ConfigCommand::Show => config::commands::show()?,
        },
        Command::Status => store::commands::status()?,
        Command::Init(ref args) => init::commands::init(args)?,
    }

    Ok(())
}
