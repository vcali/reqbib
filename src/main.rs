use clap::{Arg, Command};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const FILTERED_WORDS: &[&str] = &["curl", "http", "https", "www"];

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

fn get_data_file_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".reqbib");
    path.push("commands.json");
    path
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
            Arg::new("keywords")
                .help("Keywords to search for")
                .num_args(0..),
        )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = build_cli().get_matches();

    let data_file = get_data_file_path();
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
}
