use crate::keywords::extract_keywords;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct StoredCommand {
    pub(crate) command: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    pub(crate) keywords: Vec<String>,
}

impl StoredCommand {
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
pub(crate) struct CommandDatabase {
    pub(crate) commands: Vec<StoredCommand>,
}

impl CommandDatabase {
    pub(crate) fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub(crate) fn load_from_file(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let db: CommandDatabase = serde_json::from_str(&content)?;
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
            self.commands.push(StoredCommand::new(command, description));
            true
        }
    }

    pub(crate) fn search(&self, keywords: &[String]) -> Vec<&StoredCommand> {
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
    use super::{CommandDatabase, StoredCommand};
    use tempfile::TempDir;

    #[test]
    fn test_stored_command_new_includes_description_keywords() {
        let command = "curl https://example.com/releases".to_string();
        let stored = StoredCommand::new(command.clone(), Some("Upload release build".to_string()));

        assert_eq!(stored.command, command);
        assert_eq!(stored.description.as_deref(), Some("Upload release build"));
        assert!(stored.keywords.contains(&"upload".to_string()));
        assert!(stored.keywords.contains(&"release".to_string()));
        assert!(stored.keywords.contains(&"build".to_string()));
        assert!(stored.keywords.contains(&"example".to_string()));
    }

    #[test]
    fn test_command_database_new() {
        let db = CommandDatabase::new();
        assert!(db.commands.is_empty());
    }

    #[test]
    fn test_command_database_add_command() {
        let mut db = CommandDatabase::new();
        let command = "git log --oneline -20".to_string();

        db.add_command(command.clone(), Some("Recent history".to_string()));
        assert_eq!(db.commands.len(), 1);
        assert_eq!(db.commands[0].command, command);
        assert_eq!(
            db.commands[0].description.as_deref(),
            Some("Recent history")
        );
    }

    #[test]
    fn test_command_database_add_duplicate_command() {
        let mut db = CommandDatabase::new();
        let command = "git status".to_string();

        db.add_command(command.clone(), Some("First description".to_string()));
        db.add_command(command, Some("Second description".to_string()));

        assert_eq!(db.commands.len(), 1);
        assert_eq!(
            db.commands[0].description.as_deref(),
            Some("First description")
        );
    }

    #[test]
    fn test_command_database_search() {
        let mut db = CommandDatabase::new();

        db.add_command("git log --oneline --graph".to_string(), None);
        db.add_command("aws s3 ls s3://example-bucket".to_string(), None);
        db.add_command(
            "curl -X POST https://api.github.com/repos".to_string(),
            Some("Create repository".to_string()),
        );

        assert_eq!(db.search(&["graph".to_string()]).len(), 1);
        assert_eq!(db.search(&["bucket".to_string()]).len(), 1);
        assert_eq!(db.search(&["github".to_string()]).len(), 1);
        assert_eq!(db.search(&["repository".to_string()]).len(), 1);
        assert_eq!(db.search(&["nonexistent".to_string()]).len(), 0);
    }

    #[test]
    fn test_command_database_search_case_insensitive() {
        let mut db = CommandDatabase::new();
        db.add_command(
            "AWS S3 LS s3://Example-Bucket".to_string(),
            Some("List artifacts".to_string()),
        );

        assert_eq!(db.search(&["aws".to_string()]).len(), 1);
        assert_eq!(db.search(&["bucket".to_string()]).len(), 1);
        assert_eq!(db.search(&["ARTIFACTS".to_string()]).len(), 1);
    }

    #[test]
    fn test_command_database_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_commands.json");

        let mut db = CommandDatabase::new();
        db.add_command("git log --oneline".to_string(), Some("Example".to_string()));
        db.add_command("aws sts get-caller-identity".to_string(), None);

        db.save_to_file(&file_path).unwrap();
        assert!(file_path.exists());

        let loaded_db = CommandDatabase::load_from_file(&file_path).unwrap();
        assert_eq!(loaded_db.commands.len(), 2);
        assert_eq!(loaded_db, db);
    }

    #[test]
    fn test_command_database_load_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.json");

        let db = CommandDatabase::load_from_file(&file_path).unwrap();
        assert!(db.commands.is_empty());
    }

    #[test]
    fn test_command_database_loads_entries_without_description() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("legacy_commands.json");
        std::fs::write(
            &file_path,
            r#"{
  "commands": [
    {
      "command": "git status",
      "keywords": ["git", "status"]
    }
  ]
}"#,
        )
        .unwrap();

        let db = CommandDatabase::load_from_file(&file_path).unwrap();

        assert_eq!(db.commands.len(), 1);
        assert_eq!(db.commands[0].description, None);
    }

    #[test]
    fn test_search_partial_keyword_match() {
        let mut db = CommandDatabase::new();
        db.add_command("curl https://api.github.com/repositories".to_string(), None);

        assert_eq!(db.search(&["repo".to_string()]).len(), 1);
        assert_eq!(db.search(&["hub".to_string()]).len(), 1);
    }
}
