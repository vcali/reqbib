use clap::{Arg, Command};

pub(crate) fn build_cli() -> Command {
    Command::new("reqbib")
        .about("A CLI tool for managing curl commands")
        .version("0.1.0")
        .arg(
            Arg::new("add")
                .short('a')
                .long("add")
                .value_name("CURL_COMMAND")
                .help("Add a new curl command"),
        )
        .arg(
            Arg::new("import")
                .short('i')
                .long("import")
                .help("Import curl commands from shell history")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List all stored curl commands")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .long("config")
                .value_name("PATH")
                .help("Path to a reqbib config file"),
        )
        .arg(
            Arg::new("repo")
                .long("repo")
                .value_name("PATH")
                .help("Path to a shared GitHub repository checkout"),
        )
        .arg(
            Arg::new("teams-dir")
                .long("teams-dir")
                .value_name("PATH")
                .help("Relative path to the teams directory inside the shared repository"),
        )
        .arg(
            Arg::new("team")
                .long("team")
                .value_name("TEAM")
                .help("Team folder inside the shared repository"),
        )
        .arg(
            Arg::new("all-teams")
                .long("all-teams")
                .help("Search or list across all teams in the shared repository")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("local-only")
                .long("local-only")
                .help("Search or list only local commands")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("shared-only")
                .long("shared-only")
                .help("Search or list only shared repository commands")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("keywords")
                .help("Keywords to search for")
                .num_args(0..),
        )
}
