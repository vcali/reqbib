use clap::{Arg, Command};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug)]
struct CurlDatabase {
    commands: Vec<CurlCommand>,
}

impl CurlDatabase {
    fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let db: CurlDatabase = serde_json::from_str(&content)?;
            Ok(db)
        } else {
            Ok(Self::new())
        }
    }

    fn save_to_file(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn add_command(&mut self, command: String) {
        let curl_cmd = CurlCommand::new(command);
        
        // Check for duplicates
        if !self.commands.iter().any(|c| c.command == curl_cmd.command) {
            self.commands.push(curl_cmd);
        }
    }

    fn search(&self, keywords: &[String]) -> Vec<&CurlCommand> {
        self.commands
            .iter()
            .filter(|cmd| {
                keywords.iter().all(|keyword| {
                    let keyword_lower = keyword.to_lowercase();
                    cmd.keywords
                        .iter()
                        .any(|k| k.to_lowercase().contains(&keyword_lower))
                        || cmd.command.to_lowercase().contains(&keyword_lower)
                })
            })
            .collect()
    }
}

fn extract_keywords(command: &str) -> Vec<String> {
    let mut keywords = HashSet::new();
    
    // Extract URLs and domain names
    let url_regex = Regex::new(r"https?://([^/\s]+)").unwrap();
    for cap in url_regex.captures_iter(command) {
        if let Some(domain) = cap.get(1) {
            keywords.insert(domain.as_str().to_string());
            // Also add parts of the domain
            for part in domain.as_str().split('.') {
                if !part.is_empty() && part.len() > 2 {
                    keywords.insert(part.to_string());
                }
            }
        }
    }
    
    // Extract path segments
    let path_regex = Regex::new(r"https?://[^/\s]+/([^\s?]+)").unwrap();
    for cap in path_regex.captures_iter(command) {
        if let Some(path) = cap.get(1) {
            for segment in path.as_str().split('/') {
                if !segment.is_empty() && segment.len() > 2 {
                    keywords.insert(segment.to_string());
                }
            }
        }
    }
    
    // Extract header values and common curl flags
    let header_regex = Regex::new(r#"-H\s+["']([^"']+)["']"#).unwrap();
    for cap in header_regex.captures_iter(command) {
        if let Some(header) = cap.get(1) {
            let header_parts: Vec<&str> = header.as_str().split(':').collect();
            if header_parts.len() >= 2 {
                keywords.insert(header_parts[0].trim().to_string());
            }
        }
    }
    
    // Extract common words from the command
    let word_regex = Regex::new(r"\b[a-zA-Z]{3,}\b").unwrap();
    for cap in word_regex.find_iter(command) {
        let word = cap.as_str().to_lowercase();
        if !["curl", "http", "https", "www"].contains(&word.as_str()) {
            keywords.insert(word);
        }
    }
    
    keywords.into_iter().collect()
}

fn get_data_file_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".reqbib");
    path.push("commands.json");
    path
}

fn import_from_history() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut curl_commands = Vec::new();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    
    // Try both bash and zsh history files
    let history_files = vec![
        home.join(".bash_history"),
        home.join(".zsh_history"),
    ];
    
    let curl_regex = Regex::new(r"^(\s*curl\s+.*)$").unwrap();
    
    for history_file in history_files {
        if history_file.exists() {
            if let Ok(content) = fs::read_to_string(&history_file) {
                for line in content.lines() {
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
                    
                    if let Some(cap) = curl_regex.captures(clean_line) {
                        if let Some(curl_cmd) = cap.get(1) {
                            let cmd = curl_cmd.as_str().trim().to_string();
                            if !curl_commands.contains(&cmd) {
                                curl_commands.push(cmd);
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(curl_commands)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("reqbib")
        .about("A CLI tool for managing curl commands")
        .version("0.1.0")
        .arg(
            Arg::new("add")
                .short('a')
                .long("add")
                .value_name("CURL_COMMAND")
                .help("Add a new curl command")
        )
        .arg(
            Arg::new("import")
                .short('i')
                .long("import")
                .help("Import curl commands from shell history")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("keywords")
                .help("Keywords to search for")
                .num_args(0..)
        )
        .get_matches();

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
                let initial_count = db.commands.len();
                for cmd in commands {
                    db.add_command(cmd);
                }
                db.save_to_file(&data_file)?;
                let added_count = db.commands.len() - initial_count;
                println!("Imported {} new curl commands from shell history", added_count);
            }
            Err(e) => {
                eprintln!("Error importing from history: {}", e);
            }
        }
    } else if let Some(keywords) = matches.get_many::<String>("keywords") {
        // Search for curl commands
        let keyword_vec: Vec<String> = keywords.map(|s| s.clone()).collect();
        let results = db.search(&keyword_vec);
        
        if results.is_empty() {
            println!("No curl commands found matching keywords: {}", keyword_vec.join(" "));
        } else {
            println!("Found {} matching curl command(s):", results.len());
            for cmd in results {
                println!("{}", cmd.command);
            }
        }
    } else {
        // Show all commands if no arguments provided
        if db.commands.is_empty() {
            println!("No curl commands stored. Use 'reqbib -a <curl_command>' to add one or 'reqbib -i' to import from history.");
        } else {
            println!("All stored curl commands ({}):", db.commands.len());
            for cmd in &db.commands {
                println!("{}", cmd.command);
            }
        }
    }

    Ok(())
}
