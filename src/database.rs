use crate::keywords::extract_keywords;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct CurlCommand {
    pub(crate) command: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    pub(crate) keywords: Vec<String>,
}

impl CurlCommand {
    pub(crate) fn new(command: String, description: Option<String>) -> Self {
        let mut keywords = extract_keywords(&command);

        if let Some(description) = description.as_deref() {
            for keyword in extract_keywords(description) {
                if !keywords.contains(&keyword) {
                    keywords.push(keyword);
                }
            }
            keywords.sort();
        }

        Self {
            command,
            description,
            keywords,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct CurlDatabase {
    pub(crate) commands: Vec<CurlCommand>,
}

impl CurlDatabase {
    pub(crate) fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub(crate) fn load_from_file(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let db: CurlDatabase = serde_json::from_str(&content)?;
            Ok(db)
        } else {
            Ok(Self::new())
        }
    }

    pub(crate) fn save_to_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub(crate) fn add_command(&mut self, command: String, description: Option<String>) -> bool {
        if self
            .commands
            .iter()
            .any(|existing| existing.command == command)
        {
            false
        } else {
            self.commands.push(CurlCommand::new(command, description));
            true
        }
    }

    pub(crate) fn add_commands<I>(&mut self, commands: I) -> usize
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
                self.commands.push(CurlCommand::new(command, None));
                added_count += 1;
            }
        }

        added_count
    }

    pub(crate) fn search(&self, keywords: &[String]) -> Vec<&CurlCommand> {
        let normalized_keywords: Vec<String> = keywords
            .iter()
            .map(|keyword| keyword.to_lowercase())
            .collect();

        self.commands
            .iter()
            .filter(|cmd| {
                let command_lower = cmd.command.to_lowercase();
                let description_lower = cmd.description.as_ref().map(|value| value.to_lowercase());

                normalized_keywords.iter().all(|keyword| {
                    cmd.keywords.iter().any(|stored| stored.contains(keyword))
                        || command_lower.contains(keyword)
                        || description_lower
                            .as_ref()
                            .is_some_and(|description| description.contains(keyword))
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{CurlCommand, CurlDatabase};
    use tempfile::TempDir;

    #[test]
    fn test_curl_command_new() {
        let command =
            "curl -X POST https://api.example.com/users -H 'Content-Type: application/json'"
                .to_string();
        let curl_cmd = CurlCommand::new(command.clone(), Some("Create a user".to_string()));

        assert_eq!(curl_cmd.command, command);
        assert_eq!(curl_cmd.description.as_deref(), Some("Create a user"));
        assert!(!curl_cmd.keywords.is_empty());
        assert!(curl_cmd.keywords.contains(&"example".to_string()));
        assert!(curl_cmd.keywords.contains(&"api".to_string()));
        assert!(curl_cmd.keywords.contains(&"create".to_string()));
        assert!(curl_cmd.keywords.contains(&"user".to_string()));
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

        db.add_command(command.clone(), Some("Example endpoint".to_string()));
        assert_eq!(db.commands.len(), 1);
        assert_eq!(db.commands[0].command, command);
        assert_eq!(
            db.commands[0].description.as_deref(),
            Some("Example endpoint")
        );
    }

    #[test]
    fn test_curl_database_add_duplicate_command() {
        let mut db = CurlDatabase::new();
        let command = "curl https://example.com".to_string();

        db.add_command(command.clone(), Some("First description".to_string()));
        db.add_command(command.clone(), Some("Second description".to_string()));

        assert_eq!(db.commands.len(), 1);
        assert_eq!(
            db.commands[0].description.as_deref(),
            Some("First description")
        );
    }

    #[test]
    fn test_curl_database_search() {
        let mut db = CurlDatabase::new();

        db.add_command("curl https://api.github.com/users".to_string(), None);
        db.add_command("curl https://example.com/test".to_string(), None);
        db.add_command(
            "curl -X POST https://api.github.com/repos".to_string(),
            Some("Create repository".to_string()),
        );

        let results = db.search(&["github".to_string()]);
        assert_eq!(results.len(), 2);

        let results = db.search(&["users".to_string()]);
        assert_eq!(results.len(), 1);

        let results = db.search(&["api".to_string(), "POST".to_string()]);
        assert_eq!(results.len(), 1);

        let results = db.search(&["repository".to_string()]);
        assert_eq!(results.len(), 1);

        let results = db.search(&["nonexistent".to_string()]);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_curl_database_search_case_insensitive() {
        let mut db = CurlDatabase::new();
        db.add_command(
            "curl https://API.GitHub.com/Users".to_string(),
            Some("List users".to_string()),
        );

        let results = db.search(&["github".to_string()]);
        assert_eq!(results.len(), 1);

        let results = db.search(&["USERS".to_string()]);
        assert_eq!(results.len(), 1);

        let results = db.search(&["LIST".to_string()]);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_curl_database_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_commands.json");

        let mut db = CurlDatabase::new();
        db.add_command(
            "curl https://example.com".to_string(),
            Some("Example".to_string()),
        );
        db.add_command("curl https://github.com".to_string(), None);

        db.save_to_file(&file_path).unwrap();
        assert!(file_path.exists());

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
    fn test_curl_database_loads_legacy_entries_without_description() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("legacy_commands.json");
        std::fs::write(
            &file_path,
            r#"{
  "commands": [
    {
      "command": "curl https://example.com",
      "keywords": ["example"]
    }
  ]
}"#,
        )
        .unwrap();

        let db = CurlDatabase::load_from_file(&file_path).unwrap();

        assert_eq!(db.commands.len(), 1);
        assert_eq!(db.commands[0].description, None);
    }

    #[test]
    fn test_search_partial_keyword_match() {
        let mut db = CurlDatabase::new();
        db.add_command("curl https://api.github.com/repositories".to_string(), None);

        let results = db.search(&["repo".to_string()]);
        assert_eq!(results.len(), 1);

        let results = db.search(&["hub".to_string()]);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_curl_database_add_commands_counts_only_new_entries() {
        let mut db = CurlDatabase::new();
        db.add_command("curl https://example.com".to_string(), None);

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
