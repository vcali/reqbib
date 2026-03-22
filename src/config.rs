use crate::database::{CurlCommand, CurlDatabase};
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
const LEGACY_SHARED_REPO_CONFIG_KEYS: &[&str] = &[
    "github_repo",
    "shared_repo_path",
    "teams_dir",
    "auto_update_repo",
    "auto_update_interval_minutes",
];

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct ReqbibConfig {
    pub(crate) shared_repo: Option<SharedRepoConfig>,
    pub(crate) default_list_limit: Option<usize>,
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
struct RawReqbibConfig {
    #[serde(default)]
    shared_repo: Option<RawSharedRepoConfig>,
    default_list_limit: Option<usize>,
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

impl ReqbibConfig {
    pub(crate) fn load_from_file(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let value: Value = serde_json::from_str(&content)?;
            validate_no_legacy_flat_shared_repo_keys(&value)?;
            let config: RawReqbibConfig = serde_json::from_value(value)?;
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
        validate_relative_directory(&teams_dir)?;
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

impl TryFrom<RawReqbibConfig> for ReqbibConfig {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: RawReqbibConfig) -> Result<Self> {
        let shared_repo = match value.shared_repo {
            Some(shared_repo) => Some(SharedRepoConfig::try_from(shared_repo)?),
            None => None,
        };

        Ok(Self {
            shared_repo,
            default_list_limit: value.default_list_limit,
        })
    }
}

impl TryFrom<RawSharedRepoConfig> for SharedRepoConfig {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: RawSharedRepoConfig) -> Result<Self> {
        if let Some(teams_dir) = value.teams_dir.as_ref() {
            validate_relative_directory(teams_dir)?;
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

pub(crate) fn validate_relative_directory(path: &Path) -> Result<()> {
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

pub(crate) fn get_local_data_file_path() -> PathBuf {
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

pub(crate) fn get_team_data_file_path(
    repository_root: &Path,
    teams_dir: &Path,
    team: &str,
) -> Result<PathBuf> {
    validate_team_name(team)?;
    validate_relative_directory(teams_dir)?;
    Ok(repository_root
        .join(teams_dir)
        .join(team)
        .join("commands.json"))
}

pub(crate) fn resolve_config(matches: &ArgMatches) -> Result<ReqbibConfig> {
    let config_path = matches
        .get_one::<String>("config")
        .map(PathBuf::from)
        .unwrap_or_else(get_default_config_file_path);
    ReqbibConfig::load_from_file(&config_path)
}

pub(crate) fn resolve_shared_storage_context(
    matches: &ArgMatches,
    config: &ReqbibConfig,
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
    } else if let Some(shared_repo) = config.shared_repo.as_ref() {
        match shared_repo {
            SharedRepoConfig::Path(path_config) => path_config.path.clone(),
            SharedRepoConfig::Github(github_config) => {
                let (checkout_path, was_cloned) =
                    ensure_github_repo_checkout(&github_config.github_repo)?;
                if was_cloned {
                    let _ = write_github_repo_sync_stamp(
                        &get_default_github_state_root(),
                        &github_config.github_repo,
                    );
                } else {
                    match maybe_update_github_repo_checkout(
                        &github_config.github_repo,
                        &checkout_path,
                        github_config.auto_update_repo,
                        github_config.auto_update_interval(),
                    ) {
                        Ok(_) => {}
                        Err(error) => {
                            eprintln!(
                                "Warning: failed to update shared repository checkout: {error}"
                            );
                        }
                    }
                }
                checkout_path
            }
        }
    } else {
        if team.is_some() || all_teams {
            return Err(SHARED_REPOSITORY_REQUIRED_MESSAGE.into());
        }
        return Ok(None);
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

pub(crate) fn resolve_data_file_path(
    matches: &ArgMatches,
    shared_context: Option<&SharedStorageContext>,
) -> Result<PathBuf> {
    match matches.get_one::<String>("team") {
        Some(team) => {
            let shared_context = shared_context.ok_or(SHARED_REPOSITORY_REQUIRED_MESSAGE)?;
            get_team_data_file_path(
                &shared_context.repository_root,
                &shared_context.teams_dir,
                team,
            )
        }
        None => Ok(get_local_data_file_path()),
    }
}

pub(crate) fn load_all_team_commands(
    shared_context: &SharedStorageContext,
    keywords: Option<&[String]>,
) -> Result<Vec<(String, CurlCommand)>> {
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
    keywords: Option<&[String]>,
) -> Result<Vec<CurlCommand>> {
    let team_path = get_team_data_file_path(
        &shared_context.repository_root,
        &shared_context.teams_dir,
        team,
    )?;
    let database = CurlDatabase::load_from_file(&team_path)?;

    Ok(match keywords {
        Some(keywords) => database.search(keywords).into_iter().cloned().collect(),
        None => database.commands,
    })
}

pub(crate) fn shared_repository_required_message() -> &'static str {
    SHARED_REPOSITORY_REQUIRED_MESSAGE
}

#[cfg(test)]
mod tests {
    use super::{
        get_team_data_file_path, resolve_data_file_path, resolve_shared_storage_context,
        validate_relative_directory, DefaultSharedReadTarget, GithubSharedRepoConfig,
        PathSharedRepoConfig, ReqbibConfig, SharedRepoConfig,
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
    fn test_load_config_with_path_shared_repo() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "path",
                    "path": "/tmp/shared-reqbib",
                    "teams_dir": "company-teams"
                }
            }),
        );

        let config = ReqbibConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config,
            ReqbibConfig {
                shared_repo: Some(SharedRepoConfig::Path(PathSharedRepoConfig {
                    path: PathBuf::from("/tmp/shared-reqbib"),
                    teams_dir: Some(PathBuf::from("company-teams")),
                    default_team: None,
                    default_all_teams: false,
                })),
                default_list_limit: None,
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
                    "github_repo": "acme/shared-reqbib"
                }
            }),
        );

        let config = ReqbibConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config,
            ReqbibConfig {
                shared_repo: Some(SharedRepoConfig::Github(GithubSharedRepoConfig {
                    github_repo: "acme/shared-reqbib".to_string(),
                    teams_dir: None,
                    auto_update_repo: true,
                    auto_update_interval_minutes: DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES,
                    default_team: None,
                    default_all_teams: false,
                })),
                default_list_limit: None,
            }
        );
    }

    #[test]
    fn test_load_config_with_default_team() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "path",
                    "path": "/tmp/shared-reqbib",
                    "default_team": "platform"
                },
            }),
        );

        let config = ReqbibConfig::load_from_file(&config_path).unwrap();

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
                    "path": "/tmp/shared-reqbib",
                    "default_all_teams": true
                }
            }),
        );

        let config = ReqbibConfig::load_from_file(&config_path).unwrap();

        assert_eq!(
            config.default_shared_read_target(),
            Some(DefaultSharedReadTarget::AllTeams)
        );
    }

    #[test]
    fn test_load_config_with_default_list_limit() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "default_list_limit": 12
            }),
        );

        let config = ReqbibConfig::load_from_file(&config_path).unwrap();

        assert_eq!(config.default_list_limit, Some(12));
    }

    #[test]
    fn test_load_config_rejects_flat_legacy_shared_repo_keys() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "github_repo": "acme/shared-reqbib",
                "teams_dir": "teams"
            }),
        );

        let error = ReqbibConfig::load_from_file(&config_path)
            .expect_err("legacy flat config should be rejected");

        assert!(error
            .to_string()
            .contains("Legacy flat shared repository config is no longer supported."));
    }

    #[test]
    fn test_load_config_rejects_missing_path_for_path_mode() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "path"
                }
            }),
        );

        let error =
            ReqbibConfig::load_from_file(&config_path).expect_err("path mode requires a path");

        assert_eq!(
            error.to_string(),
            "shared_repo.mode 'path' requires shared_repo.path."
        );
    }

    #[test]
    fn test_load_config_rejects_missing_github_repo_for_github_mode() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "github"
                }
            }),
        );

        let error =
            ReqbibConfig::load_from_file(&config_path).expect_err("github mode requires a repo");

        assert_eq!(
            error.to_string(),
            "shared_repo.mode 'github' requires shared_repo.github_repo."
        );
    }

    #[test]
    fn test_load_config_rejects_mixed_path_and_github_fields() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "github",
                    "github_repo": "acme/shared-reqbib",
                    "path": "/tmp/shared-reqbib"
                }
            }),
        );

        let error = ReqbibConfig::load_from_file(&config_path)
            .expect_err("mixed config should be rejected");

        assert_eq!(
            error.to_string(),
            "shared_repo.mode 'github' cannot be combined with shared_repo.path."
        );
    }

    #[test]
    fn test_load_config_rejects_auto_update_fields_in_path_mode() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "path",
                    "path": "/tmp/shared-reqbib",
                    "auto_update_repo": true
                }
            }),
        );

        let error = ReqbibConfig::load_from_file(&config_path)
            .expect_err("path mode should reject github-only fields");

        assert_eq!(
            error.to_string(),
            "shared_repo.auto_update_repo is only valid when shared_repo.mode is 'github'."
        );
    }

    #[test]
    fn test_load_config_rejects_zero_auto_update_interval() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "github",
                    "github_repo": "acme/shared-reqbib",
                    "auto_update_interval_minutes": 0
                }
            }),
        );

        let error = ReqbibConfig::load_from_file(&config_path)
            .expect_err("zero interval should be rejected");

        assert_eq!(
            error.to_string(),
            "shared_repo.auto_update_interval_minutes must be greater than 0."
        );
    }

    #[test]
    fn test_load_config_rejects_default_team_with_default_all_teams() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "path",
                    "path": "/tmp/shared-reqbib",
                    "default_team": "platform",
                    "default_all_teams": true
                }
            }),
        );

        let error = ReqbibConfig::load_from_file(&config_path)
            .expect_err("conflicting default shared read selectors should be rejected");

        assert_eq!(
            error.to_string(),
            "shared_repo.default_team cannot be combined with shared_repo.default_all_teams."
        );
    }

    #[test]
    fn test_load_config_rejects_invalid_default_team() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_config_file(
            &temp_dir,
            serde_json::json!({
                "shared_repo": {
                    "mode": "path",
                    "path": "/tmp/shared-reqbib",
                    "default_team": "../platform"
                }
            }),
        );

        let error = ReqbibConfig::load_from_file(&config_path)
            .expect_err("invalid team names should be rejected");

        assert_eq!(
            error.to_string(),
            "Team names may only contain letters, numbers, dots, underscores, and hyphens."
        );
    }

    #[test]
    fn test_config_teams_dir_defaults_to_teams() {
        let config = ReqbibConfig::default();

        assert_eq!(config.teams_dir().unwrap(), PathBuf::from("teams"));
    }

    #[test]
    fn test_config_teams_dir_rejects_parent_directory() {
        let config = ReqbibConfig {
            shared_repo: Some(SharedRepoConfig::Path(PathSharedRepoConfig {
                path: PathBuf::from("/tmp/shared-reqbib"),
                teams_dir: Some(PathBuf::from("../teams")),
                default_team: None,
                default_all_teams: false,
            })),
            default_list_limit: None,
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
            shared_repo: Some(SharedRepoConfig::Path(PathSharedRepoConfig {
                path: PathBuf::from("/tmp/shared-reqbib"),
                teams_dir: Some(PathBuf::from("company-teams")),
                default_team: None,
                default_all_teams: false,
            })),
            default_list_limit: None,
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
            shared_repo: Some(SharedRepoConfig::Path(PathSharedRepoConfig {
                path: PathBuf::from("/tmp/shared-reqbib"),
                teams_dir: Some(PathBuf::from("company-teams")),
                default_team: None,
                default_all_teams: false,
            })),
            default_list_limit: None,
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
}
