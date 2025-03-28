use clap::{Arg, ArgAction, Command};
use miette::Result;

mod cli;
mod constants;
mod database;

use crate::cli::clean::clean;
use crate::cli::init::init;
use crate::cli::report::report;
use crate::cli::collect::collect;
use crate::constants::{CLI_ARGS_MAINTAINERS, CLI_ARGS_REPO, CLI_ARGS_PATH, CLI_ARGS_YES};

fn cli() -> Command {
    Command::new("gdfm")
        .about("A CLI for collecting and presenting data about GitHub repositories")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("init")
                .about("Initialize a new project")
                .arg(
                    Arg::new(CLI_ARGS_REPO)
                        .help("The repository to track")
                        .required(true)
                        .index(1)
                )
                .arg(
                    Arg::new(CLI_ARGS_MAINTAINERS)
                        .help("The maintainers of the repository")
                        .required(true)
                        .num_args(1..)
                        .index(2)
                )
                .arg_required_else_help(true)
        )
        .subcommand(
            Command::new("report")
                .about("Generate a report about the repository")
                .arg(
                    Arg::new(CLI_ARGS_PATH)
                        .help("The path to the repository")
                        .required(true)
                        .index(1)
                )
                .arg_required_else_help(true)
        )
        .subcommand(
            Command::new("clean")
                .about("Remove the database file")
                .arg(
                    Arg::new(CLI_ARGS_YES)
                        .short('y')
                        .long("yes")
                        .action(ArgAction::SetTrue)
                        .help("Force the removal of the database file")
                )
        )
        .subcommand(
            Command::new("collect")
                .about("Collect data for a given project")
                .arg(
                    Arg::new(CLI_ARGS_REPO)
                        .help("The repository to collect data from")
                        .required(true)
                        .index(1)
                )
                .arg_required_else_help(true)
        )
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("init", sub_matches)) => {
            init(sub_matches).await?;
        }
        Some(("report", sub_matches)) => {
            report(sub_matches).await;
        }
        Some(("clean", sub_matches)) => {
            clean(sub_matches).await?;
        }
        Some(("collect", sub_matches)) => {
            collect(sub_matches).await?;
        }
        _ => {
            unreachable!("Subcommand not found")
        }
    }

    Ok(())
}