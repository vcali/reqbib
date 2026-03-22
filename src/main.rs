use clap::{Arg, ArgMatches, Command};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

const FILTERED_WORDS: &[&str] = &["curl", "http", "https", "www"];
const GITHUB_REPO_AUTO_UPDATE_INTERVAL: Duration = Duration::from_secs(15 * 60);

fn url_regex() -> &'static Regex {
    static URL_REGEX: OnceLock<Regex> = OnceLock::new();
    URL_REGEX.get_or_init(|| Regex::new(r"https?://([^/\s]+)").expect("valid URL regex"))
}

fn path_regex() -> &'static Regex {
    static PATH_REGEX: OnceLock<Regex> = OnceLock::new();
    PATH_REGEX.get_or_init(|| Regex::new(r"https?://[^/\s]+/([^\s?]+)").expect("valid path regex"))
}

fn header_regex() -> &'static Regex {
    static HEADER_REGEX: OnceLock<Regex> = OnceLock::new();
    HEADER_REGEX.get_or_init(|| Regex::new(r#"-H\s+["']([^"']+)["']"#).expect("valid header regex"))
}

fn method_regex() -> &'static Regex {
    static METHOD_REGEX: OnceLock<Regex> = OnceLock::new();
    METHOD_REGEX.get_or_init(|| Regex::new(r"-X\s+(\w+)").expect("valid method regex"))
}

fn word_regex() -> &'static Regex {
    static WORD_REGEX: OnceLock<Regex> = OnceLock::new();
    WORD_REGEX.get_or_init(|| Regex::new(r"\b[a-zA-Z]{3,}\b").expect("valid word regex"))
}

fn history_curl_regex() -> &'static Regex {
    static HISTORY_CURL_REGEX: OnceLock<Regex> = OnceLock::new();
    HISTORY_CURL_REGEX.get_or_init(|| Regex::new(r"^(\s*curl\s+.*)$").expect("valid history regex"))
}

fn validate_team_name(team: &str) -> Result<(), Box<dyn std::error::Error>> {
    let is_valid = !team.is_empty()
        && team != "."
        && team != ".."
        && team
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));

    if is_valid {
        Ok(())
    } else {
        Err("Team names may only contain letters, numbers, dots, underscores, and hyphens.".into())
    }
}

fn validate_relative_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.as_os_str().is_empty() {
        return Err("Teams directory cannot be empty.".into());
    }

    let is_valid = path
        .components()
        .all(|component| matches!(component, Component::Normal(_)));

    if is_valid {
        Ok(())
    } else {
        Err("Teams directory must be a relative path without '.' or '..' components.".into())
    }
}

fn validate_github_repo_name(repo: &str) -> Result<(), Box<dyn std::error::Error>> {
    let Some((owner, name)) = repo.split_once('/') else {
        return Err("GitHub repository must be in the format <owner>/<repo>.".into());
    };

    let is_valid_part = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    };

    if is_valid_part(owner) && is_valid_part(name) {
        Ok(())
    } else {
        Err("GitHub repository must be in the format <owner>/<repo> using only letters, numbers, dots, underscores, and hyphens.".into())
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
struct ReqbibConfig {
    github_repo: Option<String>,
    shared_repo_path: Option<PathBuf>,
    teams_dir: Option<PathBuf>,
    auto_update_repo: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
struct SharedStorageContext {
    repository_root: PathBuf,
    teams_dir: PathBuf,
}

impl ReqbibConfig {
    fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let config: ReqbibConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    fn teams_dir(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let teams_dir = self
            .teams_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("teams"));
        validate_relative_directory(&teams_dir)?;
        Ok(teams_dir)
    }

    fn auto_update_repo(&self) -> bool {
        self.auto_update_repo.unwrap_or(true)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CurlCommand {
    command: String,
    keywords: Vec<String>,
}

impl CurlCommand {
    fn new(command: String) -> Self {
        let keywords = extract_keywords(&command);
        Self { command, keywords }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CurlDatabase {
    commands: Vec<CurlCommand>,
}

impl CurlDatabase {
    fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let db: CurlDatabase = serde_json::from_str(&content)?;
            Ok(db)
        } else {
            Ok(Self::new())
        }
    }

    fn save_to_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn add_command(&mut self, command: String) -> bool {
        if self
            .commands
            .iter()
            .any(|existing| existing.command == command)
        {
            false
        } else {
            self.commands.push(CurlCommand::new(command));
            true
        }
    }

    fn add_commands<I>(&mut self, commands: I) -> usize
    where
        I: IntoIterator<Item = String>,
    {
        let mut seen: HashSet<String> = self
            .commands
            .iter()
            .map(|cmd| cmd.command.clone())
            .collect();
        let mut added_count = 0;

        for command in commands {
            if seen.insert(command.clone()) {
                self.commands.push(CurlCommand::new(command));
                added_count += 1;
            }
        }

        added_count
    }

    fn search(&self, keywords: &[String]) -> Vec<&CurlCommand> {
        let normalized_keywords: Vec<String> = keywords
            .iter()
            .map(|keyword| keyword.to_lowercase())
            .collect();

        self.commands
            .iter()
            .filter(|cmd| {
                let command_lower = cmd.command.to_lowercase();

                normalized_keywords.iter().all(|keyword| {
                    cmd.keywords.iter().any(|stored| stored.contains(keyword))
                        || command_lower.contains(keyword)
                })
            })
            .collect()
    }
}

fn extract_keywords(command: &str) -> Vec<String> {
    let mut keywords = HashSet::new();

    // Extract URLs and domain names
    for cap in url_regex().captures_iter(command) {
        if let Some(domain) = cap.get(1) {
            let domain_str = domain.as_str().to_lowercase();
            keywords.insert(domain_str.clone());

            // Also add parts of the domain, but filter out common prefixes
            for part in domain_str.split('.') {
                if !part.is_empty() && part.len() > 2 && part != "www" {
                    keywords.insert(part.to_string());
                }
            }
        }
    }

    // Extract path segments
    for cap in path_regex().captures_iter(command) {
        if let Some(path) = cap.get(1) {
            for segment in path.as_str().split('/') {
                if !segment.is_empty() && segment.len() > 2 {
                    keywords.insert(segment.to_lowercase());
                }
            }
        }
    }

    // Extract header names and values
    for cap in header_regex().captures_iter(command) {
        if let Some(header) = cap.get(1) {
            let header_str = header.as_str();
            if let Some((header_name, header_value)) = header_str.split_once(':') {
                let header_name = header_name.trim().to_lowercase();
                if !header_name.is_empty() {
                    keywords.insert(header_name);
                }

                let value_words: Vec<&str> = header_value.split_whitespace().collect();
                for word in value_words {
                    if word.len() > 2 {
                        keywords.insert(word.to_lowercase());
                    }
                }
            }
        }
    }

    // Extract HTTP methods and common curl options
    for cap in method_regex().captures_iter(command) {
        if let Some(method) = cap.get(1) {
            keywords.insert(method.as_str().to_lowercase());
        }
    }

    // Extract common words from the command, but filter out common curl-related words
    for cap in word_regex().find_iter(command) {
        let word = cap.as_str().to_lowercase();
        if !FILTERED_WORDS.contains(&word.as_str()) {
            keywords.insert(word);
        }
    }

    let mut keywords: Vec<String> = keywords.into_iter().collect();
    keywords.sort();
    keywords
}

fn get_local_data_file_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".reqbib");
    path.push("commands.json");
    path
}

fn get_default_config_file_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".reqbib");
    path.push("config.json");
    path
}

fn get_default_github_checkout_root() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".reqbib");
    path.push("repos");
    path
}

fn get_default_github_state_root() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".reqbib");
    path.push("state");
    path
}

fn github_repo_slug(github_repo: &str) -> Result<String, Box<dyn std::error::Error>> {
    validate_github_repo_name(github_repo)?;
    let (owner, repo) = github_repo
        .split_once('/')
        .ok_or("GitHub repository must be in the format <owner>/<repo>.")?;
    Ok(format!("{owner}__{repo}"))
}

fn get_github_repo_checkout_path(
    repository_root: &Path,
    github_repo: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(repository_root.join(github_repo_slug(github_repo)?))
}

fn get_github_repo_sync_stamp_path(
    state_root: &Path,
    github_repo: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(state_root.join(format!("{}.sync", github_repo_slug(github_repo)?)))
}

fn clone_github_repo(
    github_repo: &str,
    checkout_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let gh_binary = env::var("REQBIB_GH_BIN").unwrap_or_else(|_| "gh".to_string());
    let output = ProcessCommand::new(&gh_binary)
        .arg("repo")
        .arg("clone")
        .arg(github_repo)
        .arg(checkout_path)
        .output()
        .map_err(|error| {
            format!(
                "Failed to execute '{gh_binary}'. GitHub integration requires the GitHub CLI to be installed and authenticated: {error}"
            )
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("gh repo clone failed: {}", stderr.trim()).into())
    }
}

fn pull_github_repo(checkout_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let git_binary = env::var("REQBIB_GIT_BIN").unwrap_or_else(|_| "git".to_string());
    let output = ProcessCommand::new(&git_binary)
        .arg("-C")
        .arg(checkout_path)
        .arg("pull")
        .arg("--ff-only")
        .output()
        .map_err(|error| {
            format!(
                "Failed to execute '{git_binary}'. Automatic repository updates require Git to be installed: {error}"
            )
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("git pull --ff-only failed: {}", stderr.trim()).into())
    }
}

fn should_auto_update_repo(sync_stamp_path: &Path) -> bool {
    let modified_time = fs::metadata(sync_stamp_path)
        .and_then(|metadata| metadata.modified())
        .ok();

    match modified_time {
        Some(modified_time) => match SystemTime::now().duration_since(modified_time) {
            Ok(elapsed) => elapsed >= GITHUB_REPO_AUTO_UPDATE_INTERVAL,
            Err(_) => true,
        },
        None => true,
    }
}

fn write_github_repo_sync_stamp(
    state_root: &Path,
    github_repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let sync_stamp_path = get_github_repo_sync_stamp_path(state_root, github_repo)?;
    fs::create_dir_all(state_root)?;
    fs::write(sync_stamp_path, b"updated")?;
    Ok(())
}

fn maybe_update_github_repo_checkout_with_runner<F>(
    github_repo: &str,
    checkout_path: &Path,
    auto_update_repo: bool,
    state_root: &Path,
    update_runner: F,
) -> Result<bool, Box<dyn std::error::Error>>
where
    F: FnOnce(&Path) -> Result<(), Box<dyn std::error::Error>>,
{
    if !auto_update_repo {
        return Ok(false);
    }

    let sync_stamp_path = get_github_repo_sync_stamp_path(state_root, github_repo)?;
    if !should_auto_update_repo(&sync_stamp_path) {
        return Ok(false);
    }

    update_runner(checkout_path)?;
    write_github_repo_sync_stamp(state_root, github_repo)?;
    Ok(true)
}

fn maybe_update_github_repo_checkout(
    github_repo: &str,
    checkout_path: &Path,
    auto_update_repo: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let state_root = get_default_github_state_root();
    maybe_update_github_repo_checkout_with_runner(
        github_repo,
        checkout_path,
        auto_update_repo,
        &state_root,
        pull_github_repo,
    )
}

fn ensure_github_repo_checkout_with_runner<F>(
    github_repo: &str,
    checkout_root: &Path,
    clone_runner: F,
) -> Result<(PathBuf, bool), Box<dyn std::error::Error>>
where
    F: FnOnce(&str, &Path) -> Result<(), Box<dyn std::error::Error>>,
{
    let checkout_path = get_github_repo_checkout_path(checkout_root, github_repo)?;

    if checkout_path.exists() {
        return Ok((checkout_path, false));
    }

    fs::create_dir_all(checkout_root)?;
    clone_runner(github_repo, &checkout_path)?;
    Ok((checkout_path, true))
}

fn ensure_github_repo_checkout(
    github_repo: &str,
) -> Result<(PathBuf, bool), Box<dyn std::error::Error>> {
    let checkout_root = get_default_github_checkout_root();
    ensure_github_repo_checkout_with_runner(github_repo, &checkout_root, clone_github_repo)
}

fn get_team_data_file_path(
    repository_root: &Path,
    teams_dir: &Path,
    team: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    validate_team_name(team)?;
    validate_relative_directory(teams_dir)?;
    Ok(repository_root
        .join(teams_dir)
        .join(team)
        .join("commands.json"))
}

fn resolve_config(matches: &ArgMatches) -> Result<ReqbibConfig, Box<dyn std::error::Error>> {
    let config_path = matches
        .get_one::<String>("config")
        .map(PathBuf::from)
        .unwrap_or_else(get_default_config_file_path);
    ReqbibConfig::load_from_file(&config_path)
}

fn resolve_shared_storage_context(
    matches: &ArgMatches,
    config: &ReqbibConfig,
) -> Result<Option<SharedStorageContext>, Box<dyn std::error::Error>> {
    let explicit_repo = matches.get_one::<String>("repo").map(PathBuf::from);
    let explicit_teams_dir = matches.get_one::<String>("teams-dir").map(PathBuf::from);
    let team = matches.get_one::<String>("team");
    let all_teams = matches.get_flag("all-teams");

    if team.is_some() && all_teams {
        return Err("--team cannot be used together with --all-teams.".into());
    }

    if explicit_repo.is_some() && team.is_none() && !all_teams {
        return Err("--repo requires --team when using shared repository mode.".into());
    }

    if explicit_teams_dir.is_some() && team.is_none() && !all_teams {
        return Err("--teams-dir requires --team when using shared repository mode.".into());
    }

    if team.is_none() && !all_teams {
        return Ok(None);
    }

    let repository_root = if let Some(repository_root) = explicit_repo {
        repository_root
    } else if let Some(repository_root) = config.shared_repo_path.clone() {
        repository_root
    } else if let Some(github_repo) = config.github_repo.as_deref() {
        let (checkout_path, was_cloned) = ensure_github_repo_checkout(github_repo)?;
        if was_cloned {
            let _ = write_github_repo_sync_stamp(&get_default_github_state_root(), github_repo);
        } else {
            match maybe_update_github_repo_checkout(
                github_repo,
                &checkout_path,
                config.auto_update_repo(),
            ) {
                Ok(_) => {}
                Err(error) => {
                    eprintln!("Warning: failed to update shared repository checkout: {error}");
                }
            }
        }
        checkout_path
    } else {
        return Err(
            "No shared repository configured. Use --repo, set shared_repo_path in config, or configure github_repo for gh-based checkout."
                .into(),
        );
    };

    let teams_dir = match explicit_teams_dir {
        Some(path) => {
            validate_relative_directory(&path)?;
            path
        }
        None => config.teams_dir()?,
    };

    Ok(Some(SharedStorageContext {
        repository_root,
        teams_dir,
    }))
}

fn resolve_data_file_path(
    matches: &ArgMatches,
    shared_context: Option<&SharedStorageContext>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match matches.get_one::<String>("team") {
        Some(team) => {
            let shared_context = shared_context.ok_or(
                "No shared repository configured. Use --repo, set shared_repo_path in config, or configure github_repo for gh-based checkout.",
            )?;
            get_team_data_file_path(
                &shared_context.repository_root,
                &shared_context.teams_dir,
                team,
            )
        }
        None => Ok(get_local_data_file_path()),
    }
}

fn load_all_team_commands(
    shared_context: &SharedStorageContext,
    keywords: Option<&[String]>,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let teams_root = shared_context
        .repository_root
        .join(&shared_context.teams_dir);
    if !teams_root.exists() {
        return Ok(Vec::new());
    }

    let mut team_names = Vec::new();
    for entry in fs::read_dir(&teams_root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let team_name = entry.file_name().to_string_lossy().to_string();
            if validate_team_name(&team_name).is_ok() {
                team_names.push(team_name);
            }
        }
    }
    team_names.sort();

    let mut results = Vec::new();
    for team_name in team_names {
        let team_path = get_team_data_file_path(
            &shared_context.repository_root,
            &shared_context.teams_dir,
            &team_name,
        )?;
        let database = CurlDatabase::load_from_file(&team_path)?;

        match keywords {
            Some(keywords) => {
                for command in database.search(keywords) {
                    results.push((team_name.clone(), command.command.clone()));
                }
            }
            None => {
                for command in database.commands {
                    results.push((team_name.clone(), command.command));
                }
            }
        }
    }

    Ok(results)
}

// Refactored to accept history content as a parameter for easier testing
fn parse_curl_commands_from_history(history_content: &str) -> Vec<String> {
    let mut curl_commands = Vec::new();
    let mut seen = HashSet::new();

    for line in history_content.lines() {
        // For zsh history, remove timestamp prefix if present
        let clean_line = if line.starts_with(": ") {
            if let Some(semicolon_pos) = line.find(';') {
                &line[semicolon_pos + 1..]
            } else {
                line
            }
        } else {
            line
        };

        if let Some(cap) = history_curl_regex().captures(clean_line) {
            if let Some(curl_cmd) = cap.get(1) {
                let cmd = curl_cmd.as_str().trim().to_string();
                if seen.insert(cmd.clone()) {
                    curl_commands.push(cmd);
                }
            }
        }
    }

    curl_commands
}

fn parse_curl_commands_from_history_bytes(history_content: &[u8]) -> Vec<String> {
    let history_content = String::from_utf8_lossy(history_content);
    parse_curl_commands_from_history(history_content.as_ref())
}

fn import_from_history() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    // Try both bash and zsh history files
    let history_files = [home.join(".bash_history"), home.join(".zsh_history")];

    let mut all_commands = Vec::new();
    let mut seen = HashSet::new();

    for history_file in history_files {
        if history_file.exists() {
            if let Ok(content) = fs::read(&history_file) {
                let commands = parse_curl_commands_from_history_bytes(&content);
                for cmd in commands {
                    if seen.insert(cmd.clone()) {
                        all_commands.push(cmd);
                    }
                }
            }
        }
    }

    Ok(all_commands)
}

fn build_cli() -> Command {
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
            Arg::new("keywords")
                .help("Keywords to search for")
                .num_args(0..),
        )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = build_cli().get_matches();
    let config = resolve_config(&matches)?;
    let all_teams = matches.get_flag("all-teams");
    let shared_context = resolve_shared_storage_context(&matches, &config)?;

    if all_teams && matches.get_one::<String>("add").is_some() {
        return Err("--all-teams cannot be used with --add.".into());
    }

    if all_teams && matches.get_flag("import") {
        return Err("--all-teams cannot be used with --import.".into());
    }

    let data_file = resolve_data_file_path(&matches, shared_context.as_ref())?;
    let mut db = CurlDatabase::load_from_file(&data_file)?;

    if let Some(curl_command) = matches.get_one::<String>("add") {
        // Add a new curl command
        db.add_command(curl_command.clone());
        db.save_to_file(&data_file)?;
        println!("Added curl command: {}", curl_command);
    } else if matches.get_flag("import") {
        // Import from shell history
        match import_from_history() {
            Ok(commands) => {
                let added_count = db.add_commands(commands);
                db.save_to_file(&data_file)?;
                println!(
                    "Imported {} new curl commands from shell history",
                    added_count
                );
            }
            Err(e) => {
                eprintln!("Error importing from history: {}", e);
            }
        }
    } else if matches.get_flag("list") {
        if all_teams {
            let results = load_all_team_commands(shared_context.as_ref().ok_or(
                "No shared repository configured. Use --repo, set shared_repo_path in config, or configure github_repo for gh-based checkout.",
            )?, None)?;

            if results.is_empty() {
                println!("No curl commands stored across teams.");
            } else {
                println!("All stored curl commands across teams ({}):", results.len());
                for (team, command) in results {
                    println!("[{}] {}", team, command);
                }
            }
            return Ok(());
        }

        // List all commands or filter if keywords provided
        if let Some(keywords) = matches.get_many::<String>("keywords") {
            let keyword_vec: Vec<String> = keywords.cloned().collect();
            let results = db.search(&keyword_vec);

            if results.is_empty() {
                println!(
                    "No curl commands found matching keywords: {}",
                    keyword_vec.join(" ")
                );
            } else {
                println!("Found {} matching curl command(s):", results.len());
                for cmd in results {
                    println!("{}", cmd.command);
                }
            }
        } else {
            // List all commands when no keywords provided
            if db.commands.is_empty() {
                println!("No curl commands stored. Use 'reqbib -a <curl_command>' to add one or 'reqbib -i' to import from history.");
            } else {
                println!("All stored curl commands ({}):", db.commands.len());
                for cmd in &db.commands {
                    println!("{}", cmd.command);
                }
            }
        }
    } else if let Some(keywords) = matches.get_many::<String>("keywords") {
        if all_teams {
            let keyword_vec: Vec<String> = keywords.cloned().collect();
            let results = load_all_team_commands(
                shared_context.as_ref().ok_or(
                    "No shared repository configured. Use --repo, set shared_repo_path in config, or configure github_repo for gh-based checkout.",
                )?,
                Some(&keyword_vec),
            )?;

            if results.is_empty() {
                println!(
                    "No curl commands found across teams matching keywords: {}",
                    keyword_vec.join(" ")
                );
            } else {
                println!(
                    "Found {} matching curl command(s) across teams:",
                    results.len()
                );
                for (team, command) in results {
                    println!("[{}] {}", team, command);
                }
            }
            return Ok(());
        }

        // Search for curl commands
        let keyword_vec: Vec<String> = keywords.cloned().collect();
        let results = db.search(&keyword_vec);

        if results.is_empty() {
            println!(
                "No curl commands found matching keywords: {}",
                keyword_vec.join(" ")
            );
        } else {
            println!("Found {} matching curl command(s):", results.len());
            for cmd in results {
                println!("{}", cmd.command);
            }
        }
    } else {
        // Show help when no arguments provided
        let mut cmd = build_cli();
        cmd.print_help()?;
        println!();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_curl_command_new() {
        let command =
            "curl -X POST https://api.example.com/users -H 'Content-Type: application/json'"
                .to_string();
        let curl_cmd = CurlCommand::new(command.clone());

        assert_eq!(curl_cmd.command, command);
        assert!(!curl_cmd.keywords.is_empty());
        assert!(curl_cmd.keywords.contains(&"example".to_string()));
        assert!(curl_cmd.keywords.contains(&"api".to_string()));
    }

    #[test]
    fn test_extract_keywords() {
        let command = "curl -X POST https://api.github.com/user/repos -H 'Authorization: token xyz' -d '{\"name\":\"test\"}'";
        let keywords = extract_keywords(command);

        assert!(keywords.contains(&"github".to_string()));
        assert!(keywords.contains(&"api".to_string()));
        assert!(keywords.contains(&"user".to_string()));
        assert!(keywords.contains(&"repos".to_string()));
        assert!(keywords.contains(&"authorization".to_string()));
        assert!(keywords.contains(&"post".to_string()));
        assert!(keywords.contains(&"token".to_string()));
        assert!(keywords.contains(&"name".to_string()));
        assert!(keywords.contains(&"test".to_string()));
    }

    #[test]
    fn test_extract_keywords_with_domain_parts() {
        let command = "curl https://subdomain.example.com/api/v1/data";
        let keywords = extract_keywords(command);

        assert!(keywords.contains(&"subdomain.example.com".to_string()));
        assert!(keywords.contains(&"subdomain".to_string()));
        assert!(keywords.contains(&"example".to_string()));
        assert!(keywords.contains(&"com".to_string()));
        assert!(keywords.contains(&"api".to_string()));
        assert!(keywords.contains(&"data".to_string()));
    }

    #[test]
    fn test_curl_database_new() {
        let db = CurlDatabase::new();
        assert!(db.commands.is_empty());
    }

    #[test]
    fn test_curl_database_add_command() {
        let mut db = CurlDatabase::new();
        let command = "curl https://example.com".to_string();

        db.add_command(command.clone());
        assert_eq!(db.commands.len(), 1);
        assert_eq!(db.commands[0].command, command);
    }

    #[test]
    fn test_curl_database_add_duplicate_command() {
        let mut db = CurlDatabase::new();
        let command = "curl https://example.com".to_string();

        db.add_command(command.clone());
        db.add_command(command.clone()); // Add duplicate

        assert_eq!(db.commands.len(), 1); // Should still be 1
    }

    #[test]
    fn test_curl_database_search() {
        let mut db = CurlDatabase::new();

        db.add_command("curl https://api.github.com/users".to_string());
        db.add_command("curl https://example.com/test".to_string());
        db.add_command("curl -X POST https://api.github.com/repos".to_string());

        // Search by domain
        let results = db.search(&["github".to_string()]);
        assert_eq!(results.len(), 2);

        // Search by path
        let results = db.search(&["users".to_string()]);
        assert_eq!(results.len(), 1);

        // Search by multiple keywords
        let results = db.search(&["api".to_string(), "POST".to_string()]);
        assert_eq!(results.len(), 1);

        // Search with no matches
        let results = db.search(&["nonexistent".to_string()]);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_curl_database_search_case_insensitive() {
        let mut db = CurlDatabase::new();
        db.add_command("curl https://API.GitHub.com/Users".to_string());

        let results = db.search(&["github".to_string()]);
        assert_eq!(results.len(), 1);

        let results = db.search(&["USERS".to_string()]);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_curl_database_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_commands.json");

        let mut db = CurlDatabase::new();
        db.add_command("curl https://example.com".to_string());
        db.add_command("curl https://github.com".to_string());

        // Save to file
        db.save_to_file(&file_path).unwrap();
        assert!(file_path.exists());

        // Load from file
        let loaded_db = CurlDatabase::load_from_file(&file_path).unwrap();
        assert_eq!(loaded_db.commands.len(), 2);
        assert_eq!(loaded_db, db);
    }

    #[test]
    fn test_curl_database_load_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.json");

        let db = CurlDatabase::load_from_file(&file_path).unwrap();
        assert!(db.commands.is_empty());
    }

    #[test]
    fn test_parse_curl_commands_from_bash_history() {
        let history_content = r#"ls -la
curl https://example.com
cd /home/user
curl -X POST https://api.github.com/repos
git status
  curl   https://httpbin.org/get  
echo "hello world"
curl -H "Authorization: Bearer token" https://api.example.com/data"#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 4);
        assert!(commands.contains(&"curl https://example.com".to_string()));
        assert!(commands.contains(&"curl -X POST https://api.github.com/repos".to_string()));
        assert!(commands.contains(&"curl   https://httpbin.org/get".to_string()));
        assert!(commands.contains(
            &"curl -H \"Authorization: Bearer token\" https://api.example.com/data".to_string()
        ));
    }

    #[test]
    fn test_parse_curl_commands_from_zsh_history() {
        let history_content = r#": 1647875000:0;ls -la
: 1647875010:0;curl https://example.com
: 1647875020:0;cd /home/user
: 1647875030:0;curl -X POST https://api.github.com/repos
: 1647875040:0;git status
: 1647875050:0;curl   -H "Content-Type: application/json" https://httpbin.org/post"#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 3);
        assert!(commands.contains(&"curl https://example.com".to_string()));
        assert!(commands.contains(&"curl -X POST https://api.github.com/repos".to_string()));
        assert!(commands.contains(
            &"curl   -H \"Content-Type: application/json\" https://httpbin.org/post".to_string()
        ));
    }

    #[test]
    fn test_parse_curl_commands_removes_duplicates() {
        let history_content = r#"curl https://example.com
curl https://github.com
curl https://example.com
curl https://example.com"#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 2);
        assert!(commands.contains(&"curl https://example.com".to_string()));
        assert!(commands.contains(&"curl https://github.com".to_string()));
    }

    #[test]
    fn test_parse_curl_commands_mixed_history_formats() {
        let history_content = r#"curl https://example1.com
: 1647875000:0;curl https://example2.com
curl -X POST https://example3.com
: 1647875010:0;curl -H "Auth: token" https://example4.com"#;

        let commands = parse_curl_commands_from_history(history_content);

        assert_eq!(commands.len(), 4);
        assert!(commands.contains(&"curl https://example1.com".to_string()));
        assert!(commands.contains(&"curl https://example2.com".to_string()));
        assert!(commands.contains(&"curl -X POST https://example3.com".to_string()));
        assert!(commands.contains(&"curl -H \"Auth: token\" https://example4.com".to_string()));
    }

    #[test]
    fn test_parse_curl_commands_from_non_utf8_history() {
        let history_bytes = b": 1647875000:0;curl https://example.com\n\x83\xffgarbage\n: 1647875001:0;curl -X POST https://api.github.com/repos\n";

        let commands = parse_curl_commands_from_history_bytes(history_bytes);

        assert_eq!(commands.len(), 2);
        assert!(commands.contains(&"curl https://example.com".to_string()));
        assert!(commands.contains(&"curl -X POST https://api.github.com/repos".to_string()));
    }

    #[test]
    fn test_extract_keywords_with_headers() {
        let command = r#"curl -H "Content-Type: application/json" -H "Authorization: Bearer xyz" https://api.example.com"#;
        let keywords = extract_keywords(command);

        assert!(keywords.contains(&"content-type".to_string()));
        assert!(keywords.contains(&"authorization".to_string()));
        assert!(keywords.contains(&"application".to_string()));
        assert!(keywords.contains(&"bearer".to_string()));
        assert!(keywords.contains(&"example".to_string()));
        assert!(keywords.contains(&"api".to_string()));
    }

    #[test]
    fn test_extract_keywords_filters_common_words() {
        let command = "curl https://www.example.com/api";
        let keywords = extract_keywords(command);

        // Should contain domain parts and path
        assert!(keywords.contains(&"example".to_string()));
        assert!(keywords.contains(&"api".to_string()));

        // Should not contain filtered words
        assert!(!keywords.contains(&"curl".to_string()));
        assert!(!keywords.contains(&"http".to_string()));
        assert!(!keywords.contains(&"https".to_string()));
        assert!(!keywords.contains(&"www".to_string()));
    }

    #[test]
    fn test_search_partial_keyword_match() {
        let mut db = CurlDatabase::new();
        db.add_command("curl https://api.github.com/repositories".to_string());

        // Should find with partial match
        let results = db.search(&["repo".to_string()]);
        assert_eq!(results.len(), 1);

        let results = db.search(&["hub".to_string()]);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_curl_database_add_commands_counts_only_new_entries() {
        let mut db = CurlDatabase::new();
        db.add_command("curl https://example.com".to_string());

        let added_count = db.add_commands([
            "curl https://example.com".to_string(),
            "curl https://github.com".to_string(),
            "curl https://httpbin.org/get".to_string(),
            "curl https://github.com".to_string(),
        ]);

        assert_eq!(added_count, 2);
        assert_eq!(db.commands.len(), 3);
    }

    #[test]
    fn test_get_team_data_file_path() {
        let team_path = get_team_data_file_path(
            Path::new("/tmp/shared-reqbib"),
            Path::new("teams"),
            "platform",
        )
        .unwrap();

        assert_eq!(
            team_path,
            Path::new("/tmp/shared-reqbib")
                .join("teams")
                .join("platform")
                .join("commands.json")
        );
    }

    #[test]
    fn test_get_team_data_file_path_with_custom_teams_dir() {
        let team_path = get_team_data_file_path(
            Path::new("/tmp/shared-reqbib"),
            Path::new("company-teams"),
            "platform",
        )
        .unwrap();

        assert_eq!(
            team_path,
            Path::new("/tmp/shared-reqbib")
                .join("company-teams")
                .join("platform")
                .join("commands.json")
        );
    }

    #[test]
    fn test_get_team_data_file_path_rejects_invalid_team_name() {
        let error = get_team_data_file_path(
            Path::new("/tmp/shared-reqbib"),
            Path::new("teams"),
            "../platform",
        )
        .expect_err("invalid team names should be rejected");

        assert_eq!(
            error.to_string(),
            "Team names may only contain letters, numbers, dots, underscores, and hyphens."
        );
    }

    #[test]
    fn test_load_config_from_missing_file_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("missing-config.json");

        let config = ReqbibConfig::load_from_file(&config_path).unwrap();

        assert_eq!(config, ReqbibConfig::default());
    }

    #[test]
    fn test_config_teams_dir_defaults_to_teams() {
        let config = ReqbibConfig::default();

        assert_eq!(config.teams_dir().unwrap(), PathBuf::from("teams"));
    }

    #[test]
    fn test_config_teams_dir_rejects_parent_directory() {
        let config = ReqbibConfig {
            teams_dir: Some(PathBuf::from("../teams")),
            ..ReqbibConfig::default()
        };

        let error = config
            .teams_dir()
            .expect_err("invalid teams dir should be rejected");

        assert_eq!(
            error.to_string(),
            "Teams directory must be a relative path without '.' or '..' components."
        );
    }

    #[test]
    fn test_validate_relative_directory_rejects_absolute_paths() {
        let error = validate_relative_directory(Path::new("/tmp/teams"))
            .expect_err("absolute paths should be rejected");

        assert_eq!(
            error.to_string(),
            "Teams directory must be a relative path without '.' or '..' components."
        );
    }

    #[test]
    fn test_validate_relative_directory_accepts_nested_relative_paths() {
        validate_relative_directory(Path::new("company/teams")).unwrap();
    }

    #[test]
    fn test_get_team_data_file_path_rejects_invalid_teams_dir() {
        let error = get_team_data_file_path(
            Path::new("/tmp/shared-reqbib"),
            Path::new("../teams"),
            "platform",
        )
        .expect_err("invalid teams dir should be rejected");

        assert_eq!(
            error.to_string(),
            "Teams directory must be a relative path without '.' or '..' components."
        );
    }

    #[test]
    fn test_resolve_data_file_path_uses_configured_repository_settings() {
        let command = build_cli();
        let matches = command.get_matches_from([
            "reqbib",
            "--config",
            "/tmp/config.json",
            "--team",
            "platform",
        ]);

        let config = ReqbibConfig {
            github_repo: Some("acme/shared-reqbib".to_string()),
            shared_repo_path: Some(PathBuf::from("/tmp/shared-reqbib")),
            teams_dir: Some(PathBuf::from("company-teams")),
            auto_update_repo: None,
        };

        let shared_context = resolve_shared_storage_context(&matches, &config).unwrap();
        let path = resolve_data_file_path(&matches, shared_context.as_ref()).unwrap();

        assert_eq!(
            path,
            Path::new("/tmp/shared-reqbib")
                .join("company-teams")
                .join("platform")
                .join("commands.json")
        );
    }

    #[test]
    fn test_resolve_data_file_path_prefers_cli_over_config() {
        let command = build_cli();
        let matches = command.get_matches_from([
            "reqbib",
            "--config",
            "/tmp/config.json",
            "--repo",
            "/tmp/override-repo",
            "--teams-dir",
            "override-teams",
            "--team",
            "platform",
        ]);

        let config = ReqbibConfig {
            github_repo: Some("acme/shared-reqbib".to_string()),
            shared_repo_path: Some(PathBuf::from("/tmp/shared-reqbib")),
            teams_dir: Some(PathBuf::from("company-teams")),
            auto_update_repo: None,
        };

        let shared_context = resolve_shared_storage_context(&matches, &config).unwrap();
        let path = resolve_data_file_path(&matches, shared_context.as_ref()).unwrap();

        assert_eq!(
            path,
            Path::new("/tmp/override-repo")
                .join("override-teams")
                .join("platform")
                .join("commands.json")
        );
    }

    #[test]
    fn test_validate_github_repo_name_accepts_owner_repo() {
        validate_github_repo_name("acme/shared-reqbib").unwrap();
    }

    #[test]
    fn test_validate_github_repo_name_rejects_invalid_format() {
        let error =
            validate_github_repo_name("acme").expect_err("missing repo name should be rejected");

        assert_eq!(
            error.to_string(),
            "GitHub repository must be in the format <owner>/<repo>."
        );
    }

    #[test]
    fn test_get_github_repo_checkout_path() {
        let checkout_path =
            get_github_repo_checkout_path(Path::new("/tmp/reqbib-repos"), "acme/shared-reqbib")
                .unwrap();

        assert_eq!(
            checkout_path,
            Path::new("/tmp/reqbib-repos").join("acme__shared-reqbib")
        );
    }

    #[test]
    fn test_ensure_github_repo_checkout_with_runner_clones_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_root = temp_dir.path().join("repos");

        let (checkout_path, was_cloned) = ensure_github_repo_checkout_with_runner(
            "acme/shared-reqbib",
            &checkout_root,
            |repo, destination| {
                assert_eq!(repo, "acme/shared-reqbib");
                fs::create_dir_all(destination)?;
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(checkout_path, checkout_root.join("acme__shared-reqbib"));
        assert!(checkout_path.exists());
        assert!(was_cloned);
    }

    #[test]
    fn test_ensure_github_repo_checkout_with_runner_uses_existing_checkout() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_root = temp_dir.path().join("repos");
        let existing_checkout = checkout_root.join("acme__shared-reqbib");
        fs::create_dir_all(&existing_checkout).unwrap();

        let (checkout_path, was_cloned) = ensure_github_repo_checkout_with_runner(
            "acme/shared-reqbib",
            &checkout_root,
            |_repo, _destination| Err("clone should not be called".into()),
        )
        .unwrap();

        assert_eq!(checkout_path, existing_checkout);
        assert!(!was_cloned);
    }

    #[test]
    fn test_get_github_repo_sync_stamp_path() {
        let sync_stamp_path =
            get_github_repo_sync_stamp_path(Path::new("/tmp/reqbib-state"), "acme/shared-reqbib")
                .unwrap();

        assert_eq!(
            sync_stamp_path,
            Path::new("/tmp/reqbib-state").join("acme__shared-reqbib.sync")
        );
    }

    #[test]
    fn test_maybe_update_github_repo_checkout_with_runner_updates_when_due() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_path = temp_dir.path().join("acme__shared-reqbib");
        let state_root = temp_dir.path().join("state");
        fs::create_dir_all(&checkout_path).unwrap();

        let was_updated = maybe_update_github_repo_checkout_with_runner(
            "acme/shared-reqbib",
            &checkout_path,
            true,
            &state_root,
            |path| {
                assert_eq!(path, checkout_path.as_path());
                Ok(())
            },
        )
        .unwrap();

        assert!(was_updated);
        assert!(state_root.join("acme__shared-reqbib.sync").exists());
    }

    #[test]
    fn test_maybe_update_github_repo_checkout_with_runner_respects_disable_flag() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_path = temp_dir.path().join("acme__shared-reqbib");
        let state_root = temp_dir.path().join("state");
        fs::create_dir_all(&checkout_path).unwrap();

        let was_updated = maybe_update_github_repo_checkout_with_runner(
            "acme/shared-reqbib",
            &checkout_path,
            false,
            &state_root,
            |_path| Err("update should not run".into()),
        )
        .unwrap();

        assert!(!was_updated);
        assert!(!state_root.exists());
    }

    #[test]
    fn test_maybe_update_github_repo_checkout_with_runner_skips_recent_sync() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_path = temp_dir.path().join("acme__shared-reqbib");
        let state_root = temp_dir.path().join("state");
        let sync_stamp_path = state_root.join("acme__shared-reqbib.sync");
        fs::create_dir_all(&checkout_path).unwrap();
        fs::create_dir_all(&state_root).unwrap();
        fs::write(&sync_stamp_path, b"updated").unwrap();

        let was_updated = maybe_update_github_repo_checkout_with_runner(
            "acme/shared-reqbib",
            &checkout_path,
            true,
            &state_root,
            |_path| Err("update should not run".into()),
        )
        .unwrap();

        assert!(!was_updated);
    }
}
