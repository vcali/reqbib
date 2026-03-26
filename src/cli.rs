use clap::{Arg, Command};

pub(crate) fn build_cli() -> Command {
    Command::new("combib")
        .about("A CLI tool for storing and sharing command bibliotecas")
        .version("0.1.0")
        .arg(
            Arg::new("add")
                .short('a')
                .long("add")
                .value_name("COMMAND")
                .help("Add a new command"),
        )
        .arg(
            Arg::new("description")
                .long("description")
                .value_name("TEXT")
                .help("Optional brief description for --add"),
        )
        .arg(
            Arg::new("biblioteca")
                .short('b')
                .long("biblioteca")
                .value_name("NAME")
                .help("Active biblioteca name"),
        )
        .arg(
            Arg::new("create-biblioteca")
                .long("create-biblioteca")
                .value_name("NAME")
                .help("Create a new biblioteca"),
        )
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List all stored commands in the active biblioteca")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list-bibliotecas")
                .long("list-bibliotecas")
                .help("List available bibliotecas in the active scope")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("limit")
                .long("limit")
                .value_name("COUNT")
                .value_parser(clap::value_parser!(usize))
                .help("Limit how many commands are shown with --list (0 means unlimited)"),
        )
        .arg(
            Arg::new("config")
                .long("config")
                .value_name("PATH")
                .help("Path to a combib config file"),
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
                .help("Keywords to search for in the active biblioteca")
                .num_args(0..),
        )
}
