use crate::config::{
    list_all_team_shelves, load_team_commands, validate_shelf_name, SharedStorageContext,
};
use crate::database::StoredCommand;
use crate::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BrowseData {
    pub(crate) local: Vec<ShelfData>,
    pub(crate) shared: Vec<TeamData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TeamData {
    pub(crate) team: String,
    pub(crate) shelves: Vec<ShelfData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ShelfData {
    pub(crate) shelf: String,
    pub(crate) commands: Vec<CommandEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CommandEntry {
    pub(crate) command: String,
    pub(crate) description: Option<String>,
}

impl CommandEntry {
    fn from_stored(command: StoredCommand) -> Self {
        Self {
            command: command.command,
            description: command.description,
        }
    }
}

pub(crate) fn load_browse_data_from_root(
    local_root: &Path,
    shared_context: Option<&SharedStorageContext>,
) -> Result<BrowseData> {
    let local = load_local_browse_data_from_root(local_root)?;
    let shared = match shared_context {
        Some(shared_context) => load_shared_browse_data(shared_context)?,
        None => Vec::new(),
    };

    Ok(BrowseData { local, shared })
}

pub(crate) fn load_local_browse_data_from_root(root: &Path) -> Result<Vec<ShelfData>> {
    let mut shelves = Vec::new();

    for shelf in list_shelves_in_dir(root)? {
        let path = root.join(format!("{shelf}.json"));
        let database = crate::database::CommandDatabase::load_from_file(&path)?;
        let commands = database
            .commands
            .into_iter()
            .map(CommandEntry::from_stored)
            .collect();

        shelves.push(ShelfData { shelf, commands });
    }

    Ok(shelves)
}

pub(crate) fn local_shelves_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".shellshelf")
        .join("shelves")
}

fn list_shelves_in_dir(dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut shelves = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        if validate_shelf_name(stem).is_ok() {
            shelves.push(stem.to_string());
        }
    }

    shelves.sort();
    Ok(shelves)
}

fn load_shared_browse_data(shared_context: &SharedStorageContext) -> Result<Vec<TeamData>> {
    let mut teams = Vec::new();
    let mut current_team = None::<String>;
    let mut current_shelves = Vec::new();

    for (team, shelf) in list_all_team_shelves(shared_context)? {
        if current_team.as_deref() != Some(team.as_str()) {
            if let Some(team_name) = current_team.take() {
                teams.push(TeamData {
                    team: team_name,
                    shelves: std::mem::take(&mut current_shelves),
                });
            }
            current_team = Some(team.clone());
        }

        let commands = load_team_commands(shared_context, &team, &shelf, None)?
            .into_iter()
            .map(CommandEntry::from_stored)
            .collect();
        current_shelves.push(ShelfData { shelf, commands });
    }

    if let Some(team_name) = current_team {
        teams.push(TeamData {
            team: team_name,
            shelves: current_shelves,
        });
    }

    Ok(teams)
}

#[cfg(test)]
mod tests {
    use super::{BrowseData, CommandEntry, ShelfData, TeamData};
    use crate::config::SharedStorageContext;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn write_database(path: &std::path::Path, commands: &[(&str, Option<&str>)]) {
        let values: Vec<serde_json::Value> = commands
            .iter()
            .map(|(command, description)| {
                let mut value = serde_json::Map::new();
                value.insert(
                    "command".to_string(),
                    serde_json::Value::String((*command).to_string()),
                );
                if let Some(description) = description {
                    value.insert(
                        "description".to_string(),
                        serde_json::Value::String((*description).to_string()),
                    );
                }
                value.insert("keywords".to_string(), serde_json::Value::Array(Vec::new()));
                serde_json::Value::Object(value)
            })
            .collect();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(
            path,
            serde_json::to_string_pretty(&serde_json::json!({ "commands": values })).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_load_browse_data_reads_local_and_shared_shelves() {
        let temp_dir = TempDir::new().unwrap();
        let local_root = temp_dir.path().join(".shellshelf").join("shelves");
        write_database(
            &local_root.join("curl.json"),
            &[(
                "curl https://local.example.com/health",
                Some("Local health"),
            )],
        );

        let shared_root = temp_dir.path().join("shared-shellshelf");
        write_database(
            &shared_root
                .join("teams")
                .join("platform")
                .join("shelves")
                .join("curl.json"),
            &[(
                "curl https://shared.example.com/health",
                Some("Shared health"),
            )],
        );

        let shared_context = SharedStorageContext {
            repository_root: shared_root,
            teams_dir: PathBuf::from("teams"),
        };

        let data = BrowseData {
            local: super::load_local_browse_data_from_root(&local_root).unwrap(),
            shared: super::load_shared_browse_data(&shared_context).unwrap(),
        };

        assert_eq!(
            data,
            BrowseData {
                local: vec![ShelfData {
                    shelf: "curl".to_string(),
                    commands: vec![CommandEntry {
                        command: "curl https://local.example.com/health".to_string(),
                        description: Some("Local health".to_string()),
                    }],
                }],
                shared: vec![TeamData {
                    team: "platform".to_string(),
                    shelves: vec![ShelfData {
                        shelf: "curl".to_string(),
                        commands: vec![CommandEntry {
                            command: "curl https://shared.example.com/health".to_string(),
                            description: Some("Shared health".to_string()),
                        }],
                    }],
                }],
            }
        );
    }

    #[test]
    fn test_load_browse_data_without_shared_repo_returns_local_only() {
        let temp_dir = TempDir::new().unwrap();
        let local_root = temp_dir.path().join(".shellshelf").join("shelves");
        write_database(
            &local_root.join("images.json"),
            &[("curl https://example.com/cat.gif", None)],
        );

        let data = BrowseData {
            local: super::load_local_browse_data_from_root(&local_root).unwrap(),
            shared: Vec::new(),
        };

        assert_eq!(data.shared, Vec::new());
        assert_eq!(data.local.len(), 1);
        assert_eq!(data.local[0].shelf, "images");
    }
}
