/// Module holding the `report` command
/// 
/// This module writes a simple report as an HTML file to the current working directory.

use clap::ArgMatches;

pub async fn report(matches: &ArgMatches) {
    println!("Generating a report");
    println!("{:?}", matches);
}