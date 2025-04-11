use clap::{Arg, ArgAction, Command};
use miette::Result;

mod cli;
mod constants;
mod database;

use crate::cli::clean::clean;
use crate::cli::init::init;
use crate::cli::report::report;
use crate::cli::collect::{
    collect_pull_requests,
    collect_pull_events,
    collect_pull_reviews
};
use crate::constants::{
    CLI_ARGS_REPO,
    CLI_ARGS_PATH,
    CLI_ARGS_YES,
    CLI_ARGS_NUMBER,
};

fn cli() -> Command {
    let collect = Command::new("collect")
        .about("Various commands for collecting data about a repository")
        .subcommand(
            Command::new("pulls")
                .about("Collect pull requests for a given repository")
                .arg(
                    Arg::new(CLI_ARGS_REPO)
                        .help("The repository to collect data from")
                        .required(true)
                        .index(1)
                )
                .arg_required_else_help(true)
        )
        .subcommand(
            Command::new("events")
                .about("Collect pull request events for a given repository")
                .arg(
                    Arg::new(CLI_ARGS_REPO)
                        .help("The repository to collect data from")
                        .required(true)
                        .index(1)
                )
                .arg(
                    Arg::new(CLI_ARGS_NUMBER)
                        .short('n')
                        .long(CLI_ARGS_NUMBER)
                        .help("The pull request number")
                        .action(ArgAction::Set)
                        .value_parser(is_valid_number)
                        .num_args(1..),
                )
                .arg_required_else_help(true)
        )
        .subcommand(
            Command::new("reviews")
                .about("Collect pull request reviews for a given repository")
                .arg(
                    Arg::new(CLI_ARGS_REPO)
                        .help("The repository to collect data from")
                        .required(true)
                        .index(1)
                )
                .arg(
                    Arg::new(CLI_ARGS_NUMBER)
                        .short('n')
                        .long(CLI_ARGS_NUMBER)
                        .help("The pull request number")
                        .action(ArgAction::Set)
                        .value_parser(is_valid_number)
                        .num_args(1..),
                )
                .arg_required_else_help(true)
        );

    let init = Command::new("init")
        .about("Initialize a new project")
        .arg(
            Arg::new(CLI_ARGS_REPO)
                .help("The repository to track")
                .required(true)
                .index(1)
        )
        .arg_required_else_help(true);

    let report = Command::new("report")
        .about("Generate a report about the repository")
        .arg(
            Arg::new(CLI_ARGS_PATH)
                .help("The path to the repository")
                .required(true)
                .index(1)
        )
        .arg_required_else_help(true);

    let clean = Command::new("clean")
        .about("Remove the database file")
        .arg(
            Arg::new(CLI_ARGS_YES)
                .short('y')
                .long("yes")
                .action(ArgAction::SetTrue)
                .help("Force the removal of the database file")
        );

    Command::new("gdfm")
        .about("A CLI for collecting and presenting data about GitHub repositories")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(init)
        .subcommand(report)
        .subcommand(clean)
        .subcommand(collect)
}

pub fn is_valid_number(s: &str) -> Result<u32, String> {
    if s.parse::<u32>().is_ok() {
        Ok(s.parse::<u32>().unwrap())
    } else {
        Err("Must be a positive integer".to_string())
    }
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
            match sub_matches.subcommand() {
                Some(("pulls", sub_matches)) => {
                    collect_pull_requests(sub_matches).await?;
                }
                Some(("events", sub_matches)) => {
                    collect_pull_events(sub_matches).await?;
                }
                Some(("reviews", sub_matches)) => {
                    collect_pull_reviews(sub_matches).await?;
                }
                _ => {
                    if let Some(sub_cmd) = cli().find_subcommand_mut("collect") {
                        sub_cmd.print_help().unwrap();
                    }
                }
            }
        }
        _ => {
            cli().print_help().unwrap();
        }
    }

    Ok(())
}