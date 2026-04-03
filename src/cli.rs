use clap::{Arg, Command};

pub(crate) fn build_cli() -> Command {
    Command::new("shellshelf")
        .about("A CLI for storing, searching, and sharing reusable shell commands")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("web")
                .long("web")
                .help("Run the localhost web interface")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("web-port")
                .long("web-port")
                .value_name("PORT")
                .value_parser(clap::value_parser!(u16).range(1..))
                .help("Port for the localhost web interface (overrides config, default: 4812)"),
        )
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
            Arg::new("import-postman")
                .long("import-postman")
                .value_name("PATH")
                .help("Import an exported Postman collection JSON into a new shelf"),
        )
        .arg(
            Arg::new("target-shelf")
                .long("target-shelf")
                .value_name("NAME")
                .help("Target shelf name to create when importing from Postman"),
        )
        .arg(
            Arg::new("shelf")
                .short('s')
                .long("shelf")
                .value_name("NAME")
                .help("Active shelf name"),
        )
        .arg(
            Arg::new("create-shelf")
                .long("create-shelf")
                .value_name("NAME")
                .help("Create a new shelf"),
        )
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List all stored commands in the active shelf")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list-shelves")
                .long("list-shelves")
                .help("List available shelves in the active scope")
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
                .help("Path to a shellshelf config file"),
        )
        .arg(
            Arg::new("repo")
                .long("repo")
                .value_name("PATH")
                .help("Path to a shared GitHub repository checkout"),
        )
        .arg(
            Arg::new("add-repo")
                .long("add-repo")
                .value_name("GITHUB_REPO")
                .help("Configure the shared GitHub repository from a GitHub URL or owner/repo"),
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
            Arg::new("open-pr")
                .long("open-pr")
                .help("Commit, push, and open a pull request for a shared write operation")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("base-branch")
                .long("base-branch")
                .value_name("BRANCH")
                .help("Base branch to rebase onto before opening a pull request"),
        )
        .arg(
            Arg::new("pr-branch")
                .long("pr-branch")
                .value_name("BRANCH")
                .help("Publish branch to create or reuse for --open-pr"),
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
                .help("Keywords to search for in the active shelf or across all shelves")
                .num_args(0..),
        )
}
