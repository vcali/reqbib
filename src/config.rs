use crate::database::{CommandDatabase, StoredCommand};
use crate::github::{
    ensure_github_repo_checkout, get_default_github_state_root, maybe_update_github_repo_checkout,
    validate_github_repo_name, write_github_repo_sync_stamp,
    DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES,
};
use crate::Result;
use clap::ArgMatches;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const SHARED_REPOSITORY_REQUIRED_MESSAGE: &str =
    "No shared repository configured. Use --repo or configure shared_repo in config.";
const SHELVES_DIR_NAME: &str = "shelves";
pub(crate) const BUILTIN_DEFAULT_SHELF: &str = "default";
const LEGACY_SHARED_REPO_CONFIG_KEYS: &[&str] = &[
    "github_repo",
    "shared_repo_path",
    "teams_dir",
    "auto_update_repo",
    "auto_update_interval_minutes",
];

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct ShellshelfConfig {
    pub(crate) shared_repo: Option<SharedRepoConfig>,
    pub(crate) default_list_limit: Option<usize>,
    pub(crate) default_shelf: Option<String>,
    pub(crate) web: WebConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WebTheme {
    SolarizedDark,
    SolarizedLight,
    Giphy,
    #[default]
    Dracula,
}

impl WebTheme {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::SolarizedDark => "solarized-dark",
            Self::SolarizedLight => "solarized-light",
            Self::Giphy => "giphy",
            Self::Dracula => "dracula",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct WebConfig {
    pub(crate) port: Option<u16>,
    pub(crate) theme: WebTheme,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SharedRepoConfig {
    Path(PathSharedRepoConfig),
    Github(GithubSharedRepoConfig),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PathSharedRepoConfig {
    pub(crate) path: PathBuf,
    pub(crate) teams_dir: Option<PathBuf>,
    pub(crate) default_team: Option<String>,
    pub(crate) default_all_teams: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GithubSharedRepoConfig {
    pub(crate) github_repo: String,
    pub(crate) teams_dir: Option<PathBuf>,
    pub(crate) auto_update_repo: bool,
    pub(crate) auto_update_interval_minutes: u64,
    pub(crate) default_team: Option<String>,
    pub(crate) default_all_teams: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShellshelfConfig {
    #[serde(default)]
    shared_repo: Option<RawSharedRepoConfig>,
    default_list_limit: Option<usize>,
    default_shelf: Option<String>,
    #[serde(default)]
    web: Option<RawWebConfig>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawWebConfig {
    port: Option<u16>,
    theme: Option<WebTheme>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSharedRepoConfig {
    mode: String,
    path: Option<PathBuf>,
    github_repo: Option<String>,
    teams_dir: Option<PathBuf>,
    auto_update_repo: Option<bool>,
    auto_update_interval_minutes: Option<u64>,
    default_team: Option<String>,
    default_all_teams: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SharedStorageContext {
    pub(crate) repository_root: PathBuf,
    pub(crate) teams_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DefaultSharedReadTarget {
    Team(String),
    AllTeams,
}

impl ShellshelfConfig {
    pub(crate) fn load_from_file(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let value: Value = serde_json::from_str(&content)?;
            validate_no_legacy_flat_shared_repo_keys(&value)?;
            let config: RawShellshelfConfig = serde_json::from_value(value)?;
            Self::try_from(config)
        } else {
            Ok(Self::default())
        }
    }

    pub(crate) fn teams_dir(&self) -> Result<PathBuf> {
        let teams_dir = self
            .shared_repo
            .as_ref()
            .and_then(SharedRepoConfig::teams_dir)
            .cloned()
            .unwrap_or_else(|| PathBuf::from("teams"));
        validate_relative_directory("Teams directory", &teams_dir)?;
        Ok(teams_dir)
    }

    pub(crate) fn default_shared_read_target(&self) -> Option<DefaultSharedReadTarget> {
        self.shared_repo
            .as_ref()
            .and_then(SharedRepoConfig::default_shared_read_target)
    }
}

impl SharedRepoConfig {
    fn teams_dir(&self) -> Option<&PathBuf> {
        match self {
            SharedRepoConfig::Path(config) => config.teams_dir.as_ref(),
            SharedRepoConfig::Github(config) => config.teams_dir.as_ref(),
        }
    }

    fn default_shared_read_target(&self) -> Option<DefaultSharedReadTarget> {
        match self {
            SharedRepoConfig::Path(config) => {
                default_shared_read_target(config.default_team.as_deref(), config.default_all_teams)
            }
            SharedRepoConfig::Github(config) => {
                default_shared_read_target(config.default_team.as_deref(), config.default_all_teams)
            }
        }
    }
}

fn default_shared_read_target(
    default_team: Option<&str>,
    default_all_teams: bool,
) -> Option<DefaultSharedReadTarget> {
    if default_all_teams {
        Some(DefaultSharedReadTarget::AllTeams)
    } else {
        default_team.map(|team| DefaultSharedReadTarget::Team(team.to_string()))
    }
}

impl GithubSharedRepoConfig {
    fn auto_update_interval(&self) -> Duration {
        Duration::from_secs(self.auto_update_interval_minutes.saturating_mul(60))
    }
}

impl TryFrom<RawShellshelfConfig> for ShellshelfConfig {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: RawShellshelfConfig) -> Result<Self> {
        let shared_repo = match value.shared_repo {
            Some(shared_repo) => Some(SharedRepoConfig::try_from(shared_repo)?),
            None => None,
        };

        if let Some(default_shelf) = value.default_shelf.as_deref() {
            validate_shelf_name(default_shelf)?;
        }

        Ok(Self {
            shared_repo,
            default_list_limit: value.default_list_limit,
            default_shelf: value.default_shelf,
            web: WebConfig::try_from(value.web.unwrap_or_default())?,
        })
    }
}

impl TryFrom<RawWebConfig> for WebConfig {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: RawWebConfig) -> Result<Self> {
        if value.port == Some(0) {
            return Err("web.port must be greater than 0.".into());
        }

        Ok(Self {
            port: value.port,
            theme: value.theme.unwrap_or_default(),
        })
    }
}

impl TryFrom<RawSharedRepoConfig> for SharedRepoConfig {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: RawSharedRepoConfig) -> Result<Self> {
        if let Some(teams_dir) = value.teams_dir.as_ref() {
            validate_relative_directory("Teams directory", teams_dir)?;
        }

        if let Some(default_team) = value.default_team.as_deref() {
            validate_team_name(default_team)?;
        }

        let default_all_teams = value.default_all_teams.unwrap_or(false);
        if default_all_teams && value.default_team.is_some() {
            return Err(
                "shared_repo.default_team cannot be combined with shared_repo.default_all_teams."
                    .into(),
            );
        }

        match value.mode.as_str() {
            "path" => {
                let path = value
                    .path
                    .ok_or("shared_repo.mode 'path' requires shared_repo.path.")?;

                if path.as_os_str().is_empty() {
                    return Err("shared_repo.path cannot be empty.".into());
                }

                if value.github_repo.is_some() {
                    return Err(
                        "shared_repo.mode 'path' cannot be combined with shared_repo.github_repo."
                            .into(),
                    );
                }

                if value.auto_update_repo.is_some() {
                    return Err(
                        "shared_repo.auto_update_repo is only valid when shared_repo.mode is 'github'."
                            .into(),
                    );
                }

                if value.auto_update_interval_minutes.is_some() {
                    return Err(
                        "shared_repo.auto_update_interval_minutes is only valid when shared_repo.mode is 'github'."
                            .into(),
                    );
                }

                Ok(SharedRepoConfig::Path(PathSharedRepoConfig {
                    path,
                    teams_dir: value.teams_dir,
                    default_team: value.default_team,
                    default_all_teams,
                }))
            }
            "github" => {
                let github_repo = value
                    .github_repo
                    .ok_or("shared_repo.mode 'github' requires shared_repo.github_repo.")?;
                validate_github_repo_name(&github_repo)?;

                if value.path.is_some() {
                    return Err(
                        "shared_repo.mode 'github' cannot be combined with shared_repo.path."
                            .into(),
                    );
                }

                let auto_update_interval_minutes = value
                    .auto_update_interval_minutes
                    .unwrap_or(DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES);

                if auto_update_interval_minutes == 0 {
                    return Err(
                        "shared_repo.auto_update_interval_minutes must be greater than 0.".into(),
                    );
                }

                Ok(SharedRepoConfig::Github(GithubSharedRepoConfig {
                    github_repo,
                    teams_dir: value.teams_dir,
                    auto_update_repo: value.auto_update_repo.unwrap_or(true),
                    auto_update_interval_minutes,
                    default_team: value.default_team,
                    default_all_teams,
                }))
            }
            _ => Err("shared_repo.mode must be either 'path' or 'github'.".into()),
        }
    }
}

fn validate_no_legacy_flat_shared_repo_keys(value: &Value) -> Result<()> {
    let Value::Object(object) = value else {
        return Ok(());
    };

    let legacy_keys: Vec<&str> = LEGACY_SHARED_REPO_CONFIG_KEYS
        .iter()
        .copied()
        .filter(|key| object.contains_key(*key))
        .collect();

    if legacy_keys.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Legacy flat shared repository config is no longer supported. Move {} under 'shared_repo'.",
            legacy_keys.join(", ")
        )
        .into())
    }
}

pub(crate) fn validate_team_name(team: &str) -> Result<()> {
    validate_slug(team, "Team names")
}

pub(crate) fn validate_shelf_name(shelf: &str) -> Result<()> {
    validate_slug(shelf, "Shelf names")
}

fn validate_slug(value: &str, label: &str) -> Result<()> {
    let is_valid = !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));

    if is_valid {
        Ok(())
    } else {
        Err(
            format!("{label} may only contain letters, numbers, dots, underscores, and hyphens.")
                .into(),
        )
    }
}

pub(crate) fn validate_relative_directory(label: &str, path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(format!("{label} cannot be empty.").into());
    }

    let is_valid = path
        .components()
        .all(|component| matches!(component, Component::Normal(_)));

    if is_valid {
        Ok(())
    } else {
        Err(format!("{label} must be a relative path without '.' or '..' components.").into())
    }
}

pub(crate) fn resolve_active_shelf(
    matches: &ArgMatches,
    config: &ShellshelfConfig,
) -> Result<String> {
    if let Some(shelf) = matches.get_one::<String>("shelf") {
        validate_shelf_name(shelf)?;
        Ok(shelf.clone())
    } else if let Some(default_shelf) = config.default_shelf.as_ref() {
        Ok(default_shelf.clone())
    } else {
        Ok(BUILTIN_DEFAULT_SHELF.to_string())
    }
}

pub(crate) fn get_local_data_file_path(shelf: &str) -> Result<PathBuf> {
    validate_shelf_name(shelf)?;

    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".shellshelf");
    path.push(SHELVES_DIR_NAME);
    path.push(format!("{shelf}.json"));
    Ok(path)
}

fn get_local_shelves_root() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".shellshelf");
    path.push(SHELVES_DIR_NAME);
    path
}

fn get_default_config_file_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".shellshelf");
    path.push("config.json");
    path
}

pub(crate) fn get_team_data_file_path(
    repository_root: &Path,
    teams_dir: &Path,
    team: &str,
    shelf: &str,
) -> Result<PathBuf> {
    validate_team_name(team)?;
    validate_shelf_name(shelf)?;
    validate_relative_directory("Teams directory", teams_dir)?;

    Ok(repository_root
        .join(teams_dir)
        .join(team)
        .join(SHELVES_DIR_NAME)
        .join(format!("{shelf}.json")))
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

pub(crate) fn resolve_config(matches: &ArgMatches) -> Result<ShellshelfConfig> {
    let config_path = matches
        .get_one::<String>("config")
        .map(PathBuf::from)
        .unwrap_or_else(get_default_config_file_path);
    ShellshelfConfig::load_from_file(&config_path)
}

pub(crate) fn resolve_shared_storage_context(
    matches: &ArgMatches,
    config: &ShellshelfConfig,
) -> Result<Option<SharedStorageContext>> {
    let explicit_repo = matches.get_one::<String>("repo").map(PathBuf::from);
    let explicit_teams_dir = matches.get_one::<String>("teams-dir").map(PathBuf::from);
    let team = matches.get_one::<String>("team");
    let all_teams = matches.get_flag("all-teams");

    if team.is_some() && all_teams {
        return Err("--team cannot be used together with --all-teams.".into());
    }

    let repository_root = if let Some(repository_root) = explicit_repo {
        repository_root
    } else if let Some(repository_root) = resolve_repository_root_from_config(config)? {
        repository_root
    } else {
        if team.is_some() || all_teams {
            return Err(SHARED_REPOSITORY_REQUIRED_MESSAGE.into());
        }
        return Ok(None);
    };

    let teams_dir = match explicit_teams_dir {
        Some(path) => {
            validate_relative_directory("Teams directory", &path)?;
            path
        }
        None => config.teams_dir()?,
    };

    Ok(Some(SharedStorageContext {
        repository_root,
        teams_dir,
    }))
}

fn resolve_repository_root_from_config(config: &ShellshelfConfig) -> Result<Option<PathBuf>> {
    match config.shared_repo.as_ref() {
        Some(SharedRepoConfig::Path(path_config)) => Ok(Some(path_config.path.clone())),
        Some(SharedRepoConfig::Github(github_config)) => {
            let (checkout_path, was_cloned) =
                ensure_github_repo_checkout(&github_config.github_repo)?;
            if was_cloned {
                let _ = write_github_repo_sync_stamp(
                    &get_default_github_state_root(),
                    &github_config.github_repo,
                );
            } else if let Err(error) = maybe_update_github_repo_checkout(
                &github_config.github_repo,
                &checkout_path,
                github_config.auto_update_repo,
                github_config.auto_update_interval(),
            ) {
                eprintln!("Warning: failed to update shared repository checkout: {error}");
            }
            Ok(Some(checkout_path))
        }
        None => Ok(None),
    }
}

pub(crate) fn resolve_data_file_path(
    matches: &ArgMatches,
    shared_context: Option<&SharedStorageContext>,
    shelf: &str,
) -> Result<PathBuf> {
    match matches.get_one::<String>("team") {
        Some(team) => {
            let shared_context = shared_context.ok_or(SHARED_REPOSITORY_REQUIRED_MESSAGE)?;
            get_team_data_file_path(
                &shared_context.repository_root,
                &shared_context.teams_dir,
                team,
                shelf,
            )
        }
        None => get_local_data_file_path(shelf),
    }
}

pub(crate) fn list_local_shelves() -> Result<Vec<String>> {
    list_shelves_in_dir(&get_local_shelves_root())
}

pub(crate) fn list_team_shelves(
    shared_context: &SharedStorageContext,
    team: &str,
) -> Result<Vec<String>> {
    validate_team_name(team)?;
    let shelves_dir = shared_context
        .repository_root
        .join(&shared_context.teams_dir)
        .join(team)
        .join(SHELVES_DIR_NAME);
    list_shelves_in_dir(&shelves_dir)
}

pub(crate) fn list_all_team_shelves(
    shared_context: &SharedStorageContext,
) -> Result<Vec<(String, String)>> {
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
        for shelf in list_team_shelves(shared_context, &team_name)? {
            results.push((team_name.clone(), shelf));
        }
    }

    Ok(results)
}

pub(crate) fn load_all_team_commands(
    shared_context: &SharedStorageContext,
    shelf: &str,
    keywords: Option<&[String]>,
) -> Result<Vec<(String, StoredCommand)>> {
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
            shelf,
        )?;
        let database = CommandDatabase::load_from_file(&team_path)?;

        match keywords {
            Some(keywords) => {
                for command in database.search_in_shelf(keywords, shelf) {
                    results.push((team_name.clone(), command.clone()));
                }
            }
            None => {
                for command in database.commands {
                    results.push((team_name.clone(), command));
                }
            }
        }
    }

    Ok(results)
}

pub(crate) fn load_team_commands(
    shared_context: &SharedStorageContext,
    team: &str,
    shelf: &str,
    keywords: Option<&[String]>,
) -> Result<Vec<StoredCommand>> {
    let team_path = get_team_data_file_path(
        &shared_context.repository_root,
        &shared_context.teams_dir,
        team,
        shelf,
    )?;
    let database = CommandDatabase::load_from_file(&team_path)?;

    Ok(match keywords {
        Some(keywords) => database
            .search_in_shelf(keywords, shelf)
            .into_iter()
            .cloned()
            .collect(),
        None => database.commands,
    })
}

pub(crate) fn shared_repository_required_message() -> &'static str {
    SHARED_REPOSITORY_REQUIRED_MESSAGE
}

#[cfg(test)]
mod tests {
    use super::{
        get_local_data_file_path, get_team_data_file_path, list_all_team_shelves,
        list_local_shelves, list_team_shelves, resolve_active_shelf, validate_relative_directory,
        validate_shelf_name, DefaultSharedReadTarget, GithubSharedRepoConfig, PathSharedRepoConfig,
        SharedRepoConfig, SharedStorageContext, ShellshelfConfig, WebConfig, WebTheme,
        BUILTIN_DEFAULT_SHELF,
    };
    use crate::cli::build_cli;
    use crate::github::DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn write_config_file(temp_dir: &TempDir, value: serde_json::Value) -> PathBuf {
        let config_path = temp_dir.path().join("config.json");
        fs::write(
            &config_path,
            serde_json::to_string_pretty(&value).expect("valid JSON"),
        )
        .expect("config file should be written");
        config_path
    }

    #[test]
    fn test_validate_shelf_name() {
        validate_shelf_name("curl").unwrap();
        validate_shelf_name("git-tools").unwrap();

        let error = validate_shelf_name("../bad").expect_err("invalid shelf");
        assert_eq!(
            error.to_string(),
            "Shelf names may only contain letters, numbers, dots, underscores, and hyphens."
        );
    }

    #[test]
    fn test_validate_relative_directory_rejects_parent_components() {
        let error = validate_relative_directory("Teams directory", Path::new("../teams"))
            .expect_err("invalid teams dir");

        assert_eq!(
            error.to_string(),
            "Teams directory must be a relative path without '.' or '..' components."
        );
    }

    #[test]
    fn test_get_local_data_file_path() {
        let path = get_local_data_file_path("curl").unwrap();
        assert!(path.ends_with(".shellshelf/shelves/curl.json"));
    }

    #[test]
    fn test_get_team_data_file_path() {
        let team_path = get_team_data_file_path(
            Path::new("/tmp/shared-shellshelf"),
            Path::new("teams"),
            "platform",
            "curl",
        )
        .unwrap();

        assert_eq!(
            team_path,
            Path::new("/tmp/shared-shellshelf")
                .join("teams")
                .join("platform")
                .join("shelves")
                .join("curl.json")
        );
    }

    #[test]
    fn test_list_local_shelves() {
        let temp_dir = TempDir::new().unwrap();
        let original_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp_dir.path());

        fs::create_dir_all(temp_dir.path().join(".shellshelf").join("shelves")).unwrap();
        fs::write(
            temp_dir
                .path()
                .join(".shellshelf")
                .join("shelves")
                .join("curl.json"),
            "{}",
        )
        .unwrap();
        fs::write(
            temp_dir
                .path()
                .join(".shellshelf")
                .join("shelves")
                .join("git.json"),
            "{}",
        )
        .unwrap();
        fs::write(
            temp_dir
                .path()
                .join(".shellshelf")
                .join("shelves")
                .join("README.txt"),
            "",
        )
        .unwrap();

        let shelves = list_local_shelves().unwrap();

        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }

        assert_eq!(shelves, vec!["curl".to_string(), "git".to_string()]);
    }

    #[test]
    fn test_list_team_shelves() {
        let temp_dir = TempDir::new().unwrap();
        let shared_context = SharedStorageContext {
            repository_root: temp_dir.path().join("shared-shellshelf"),
            teams_dir: PathBuf::from("teams"),
        };
        fs::create_dir_all(
            shared_context
                .repository_root
                .join("teams")
                .join("platform")
                .join("shelves"),
        )
        .unwrap();
        fs::write(
            shared_context
                .repository_root
                .join("teams")
                .join("platform")
                .join("shelves")
                .join("curl.json"),
            "{}",
        )
        .unwrap();

        let shelves = list_team_shelves(&shared_context, "platform").unwrap();

        assert_eq!(shelves, vec!["curl".to_string()]);
    }

    #[test]
    fn test_list_all_team_shelves() {
        let temp_dir = TempDir::new().unwrap();
        let shared_context = SharedStorageContext {
            repository_root: temp_dir.path().join("shared-shellshelf"),
            teams_dir: PathBuf::from("teams"),
        };

        for (team, shelf) in [("payments", "curl"), ("platform", "aws")] {
            let dir = shared_context
                .repository_root
                .join("teams")
                .join(team)
                .join("shelves");
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(format!("{shelf}.json")), "{}").unwrap();
        }

        let shelves = list_all_team_shelves(&shared_context).unwrap();

        assert_eq!(
            shelves,
            vec![
                ("payments".to_string(), "curl".to_string()),
                ("platform".to_string(), "aws".to_string())
            ]
        );
    }

    #[test]
    fn test_load_config_from_missing_file_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("missing-config.json");

        let config = ShellshelfConfig::load_from_file(&config_path).unwrap();

        assert_eq!(config, ShellshelfConfig::default());
    }

    #[test]
    fn test_load_config_with_path_shared_repo_and_default_shelf() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "default_shelf": "curl",
                "shared_repo": {
                    "mode": "path",
                    "path": "/tmp/shared-shellshelf",
                    "teams_dir": "company-teams"
                }
            }),
        );

        let config = ShellshelfConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config,
            ShellshelfConfig {
                shared_repo: Some(SharedRepoConfig::Path(PathSharedRepoConfig {
                    path: PathBuf::from("/tmp/shared-shellshelf"),
                    teams_dir: Some(PathBuf::from("company-teams")),
                    default_team: None,
                    default_all_teams: false,
                })),
                default_list_limit: None,
                default_shelf: Some("curl".to_string()),
                web: WebConfig::default(),
            }
        );
    }

    #[test]
    fn test_load_config_with_github_shared_repo_and_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "github",
                    "github_repo": "acme/shared-shellshelf"
                }
            }),
        );

        let config = ShellshelfConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config,
            ShellshelfConfig {
                shared_repo: Some(SharedRepoConfig::Github(GithubSharedRepoConfig {
                    github_repo: "acme/shared-shellshelf".to_string(),
                    teams_dir: None,
                    auto_update_repo: true,
                    auto_update_interval_minutes: DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES,
                    default_team: None,
                    default_all_teams: false,
                })),
                default_list_limit: None,
                default_shelf: None,
                web: WebConfig::default(),
            }
        );
    }

    #[test]
    fn test_load_config_with_web_theme_and_port() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "web": {
                    "port": 4940,
                    "theme": "giphy"
                }
            }),
        );

        let config = ShellshelfConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config.web,
            WebConfig {
                port: Some(4940),
                theme: WebTheme::Giphy,
            }
        );
    }

    #[test]
    fn test_load_config_with_dracula_web_theme() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "web": {
                    "theme": "dracula"
                }
            }),
        );

        let config = ShellshelfConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config.web,
            WebConfig {
                port: None,
                theme: WebTheme::Dracula,
            }
        );
    }

    #[test]
    fn test_load_config_rejects_web_port_zero() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "web": {
                    "port": 0
                }
            }),
        );

        let error =
            ShellshelfConfig::load_from_file(&config_path).expect_err("web.port zero should fail");

        assert_eq!(error.to_string(), "web.port must be greater than 0.");
    }

    #[test]
    fn test_load_config_with_default_team() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "path",
                    "path": "/tmp/shared-shellshelf",
                    "default_team": "platform"
                }
            }),
        );

        let config = ShellshelfConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config.default_shared_read_target(),
            Some(DefaultSharedReadTarget::Team("platform".to_string()))
        );
    }

    #[test]
    fn test_load_config_with_default_all_teams() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "path",
                    "path": "/tmp/shared-shellshelf",
                    "default_all_teams": true
                }
            }),
        );

        let config = ShellshelfConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config.default_shared_read_target(),
            Some(DefaultSharedReadTarget::AllTeams)
        );
    }

    #[test]
    fn test_load_config_rejects_flat_legacy_shared_repo_keys() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "github_repo": "acme/shared-shellshelf",
                "teams_dir": "teams"
            }),
        );

        let error = ShellshelfConfig::load_from_file(&config_path)
            .expect_err("legacy flat config should be rejected");

        assert!(error
            .to_string()
            .contains("Legacy flat shared repository config is no longer supported."));
    }

    #[test]
    fn test_resolve_active_shelf_prefers_cli_then_config() {
        let matches = build_cli()
            .try_get_matches_from(["shellshelf", "-s", "git"])
            .unwrap();
        let config = ShellshelfConfig {
            default_shelf: Some("curl".to_string()),
            ..ShellshelfConfig::default()
        };

        assert_eq!(resolve_active_shelf(&matches, &config).unwrap(), "git");

        let matches = build_cli()
            .try_get_matches_from(["shellshelf", "-l"])
            .unwrap();
        assert_eq!(resolve_active_shelf(&matches, &config).unwrap(), "curl");
    }

    #[test]
    fn test_resolve_active_shelf_falls_back_to_builtin_default() {
        let matches = build_cli()
            .try_get_matches_from(["shellshelf", "-l"])
            .unwrap();
        let config = ShellshelfConfig::default();

        assert_eq!(
            resolve_active_shelf(&matches, &config).unwrap(),
            BUILTIN_DEFAULT_SHELF
        );
    }
}
