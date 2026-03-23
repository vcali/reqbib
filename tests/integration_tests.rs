use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn write_path_config(
    config_path: &Path,
    shared_repo: &Path,
    teams_dir: &str,
    default_biblioteca: Option<&str>,
    default_team: Option<&str>,
    default_all_teams: Option<bool>,
    default_list_limit: Option<usize>,
) {
    let mut shared_repo_value = serde_json::Map::new();
    shared_repo_value.insert(
        "mode".to_string(),
        serde_json::Value::String("path".to_string()),
    );
    shared_repo_value.insert(
        "path".to_string(),
        serde_json::Value::String(shared_repo.display().to_string()),
    );
    shared_repo_value.insert(
        "teams_dir".to_string(),
        serde_json::Value::String(teams_dir.to_string()),
    );
    if let Some(default_team) = default_team {
        shared_repo_value.insert(
            "default_team".to_string(),
            serde_json::Value::String(default_team.to_string()),
        );
    }
    if let Some(default_all_teams) = default_all_teams {
        shared_repo_value.insert(
            "default_all_teams".to_string(),
            serde_json::Value::Bool(default_all_teams),
        );
    }

    let mut config = serde_json::Map::new();
    config.insert(
        "shared_repo".to_string(),
        serde_json::Value::Object(shared_repo_value),
    );
    if let Some(default_biblioteca) = default_biblioteca {
        config.insert(
            "default_biblioteca".to_string(),
            serde_json::Value::String(default_biblioteca.to_string()),
        );
    }
    if let Some(default_list_limit) = default_list_limit {
        config.insert(
            "default_list_limit".to_string(),
            serde_json::Value::Number(default_list_limit.into()),
        );
    }

    fs::write(
        config_path,
        serde_json::to_string_pretty(&serde_json::Value::Object(config)).unwrap(),
    )
    .unwrap();
}

#[derive(Default)]
struct GithubConfigOptions<'a> {
    default_biblioteca: Option<&'a str>,
    default_team: Option<&'a str>,
    default_all_teams: Option<bool>,
    auto_update_repo: Option<bool>,
    auto_update_interval_minutes: Option<u64>,
}

fn write_github_config(
    config_path: &Path,
    github_repo: &str,
    teams_dir: &str,
    options: GithubConfigOptions<'_>,
) {
    let mut shared_repo = serde_json::Map::new();
    shared_repo.insert(
        "mode".to_string(),
        serde_json::Value::String("github".to_string()),
    );
    shared_repo.insert(
        "github_repo".to_string(),
        serde_json::Value::String(github_repo.to_string()),
    );
    shared_repo.insert(
        "teams_dir".to_string(),
        serde_json::Value::String(teams_dir.to_string()),
    );
    if let Some(default_team) = options.default_team {
        shared_repo.insert(
            "default_team".to_string(),
            serde_json::Value::String(default_team.to_string()),
        );
    }
    if let Some(default_all_teams) = options.default_all_teams {
        shared_repo.insert(
            "default_all_teams".to_string(),
            serde_json::Value::Bool(default_all_teams),
        );
    }
    if let Some(auto_update_repo) = options.auto_update_repo {
        shared_repo.insert(
            "auto_update_repo".to_string(),
            serde_json::Value::Bool(auto_update_repo),
        );
    }
    if let Some(auto_update_interval_minutes) = options.auto_update_interval_minutes {
        shared_repo.insert(
            "auto_update_interval_minutes".to_string(),
            serde_json::Value::Number(auto_update_interval_minutes.into()),
        );
    }

    let mut config = serde_json::Map::new();
    config.insert(
        "shared_repo".to_string(),
        serde_json::Value::Object(shared_repo),
    );
    if let Some(default_biblioteca) = options.default_biblioteca {
        config.insert(
            "default_biblioteca".to_string(),
            serde_json::Value::String(default_biblioteca.to_string()),
        );
    }

    fs::write(
        config_path,
        serde_json::to_string_pretty(&serde_json::Value::Object(config)).unwrap(),
    )
    .unwrap();
}

fn write_mock_gh(temp_dir: &Path) -> (PathBuf, PathBuf) {
    let log_path = temp_dir.join("gh.log");
    let gh_path = if cfg!(windows) {
        temp_dir.join("gh.cmd")
    } else {
        temp_dir.join("gh")
    };

    let script = if cfg!(windows) {
        format!(
            "@echo off\r\n\
setlocal\r\n\
echo %* > \"{}\"\r\n\
mkdir \"%4\" >nul 2>nul\r\n",
            log_path.display()
        )
    } else {
        format!(
            "#!/bin/sh\n\
printf '%s\\n' \"$@\" > \"{}\"\n\
mkdir -p \"$4\"\n",
            log_path.display()
        )
    };

    fs::write(&gh_path, script).unwrap();

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&gh_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&gh_path, permissions).unwrap();
    }

    (gh_path, log_path)
}

fn write_mock_git(temp_dir: &Path) -> (PathBuf, PathBuf) {
    let log_path = temp_dir.join("git.log");
    let git_path = if cfg!(windows) {
        temp_dir.join("git.cmd")
    } else {
        temp_dir.join("git")
    };

    let script = if cfg!(windows) {
        format!(
            "@echo off\r\n\
setlocal\r\n\
echo %* > \"{}\"\r\n",
            log_path.display()
        )
    } else {
        format!(
            "#!/bin/sh\n\
printf '%s\\n' \"$@\" > \"{}\"\n",
            log_path.display()
        )
    };

    fs::write(&git_path, script).unwrap();

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&git_path, permissions).unwrap();
    }

    (git_path, log_path)
}

fn write_command_database(path: &Path, commands: &[(&str, Option<&str>)]) {
    let values: Vec<serde_json::Value> = commands
        .iter()
        .map(|(command, description)| {
            let mut keywords = vec![];
            for word in command.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-') {
                let word = word.to_lowercase();
                if word.len() > 2 && !keywords.contains(&word) {
                    keywords.push(word);
                }
            }
            if let Some(description) = description {
                for word in description.split_whitespace() {
                    let word = word.to_lowercase();
                    if word.len() > 2 && !keywords.contains(&word) {
                        keywords.push(word);
                    }
                }
            }

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
            value.insert(
                "keywords".to_string(),
                serde_json::Value::Array(
                    keywords
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
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
fn test_help_output() {
    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "A CLI tool for storing and sharing command bibliotecas",
        ))
        .stdout(predicate::str::contains("Usage: combib"))
        .stdout(predicate::str::contains("--biblioteca"))
        .stdout(predicate::str::contains("--list"))
        .stdout(predicate::str::contains("--import").not());
}

#[test]
fn test_add_requires_biblioteca_without_default() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("combib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--add")
        .arg("curl https://example.com");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No biblioteca selected"));
}

#[test]
fn test_add_command_local() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("combib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["-b", "curl", "--add", "curl https://example.com/test"]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Added command to biblioteca 'curl': curl https://example.com/test",
    ));

    let data_file = temp_dir
        .path()
        .join(".combib")
        .join("libs")
        .join("curl.json");
    assert!(data_file.exists());

    let content = fs::read_to_string(data_file).unwrap();
    assert!(content.contains("curl https://example.com/test"));
}

#[test]
fn test_add_command_with_description() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("combib").unwrap();

    cmd.env("HOME", temp_dir.path()).args([
        "-b",
        "curl",
        "--add",
        "curl https://example.com/test",
        "--description",
        "Example request",
    ]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Added command to biblioteca 'curl': curl https://example.com/test (Example request)",
    ));
}

#[test]
fn test_list_empty_biblioteca() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("combib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["-b", "curl", "--list"]);

    cmd.assert().success().stdout(predicate::str::contains(
        "No commands stored in biblioteca 'curl'.",
    ));
}

#[test]
fn test_list_with_commands() {
    let temp_dir = TempDir::new().unwrap();
    let multiline_command = "curl -X POST https://api.example.com/graphql \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\"query\":\"{ viewer { login } }\"}'";

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-b", "curl", "--add", multiline_command]);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-b", "curl", "--list"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL / CURL ==="))
        .stdout(predicate::str::contains("[1]"))
        .stdout(predicate::str::contains(
            "curl -X POST https://api.example.com/graphql \\",
        ));
}

#[test]
fn test_search_commands() {
    let temp_dir = TempDir::new().unwrap();

    for command in [
        "curl https://api.github.com/users",
        "curl https://example.com/test",
        "curl -X POST https://api.github.com/repos",
    ] {
        let mut cmd = Command::cargo_bin("combib").unwrap();
        cmd.env("HOME", temp_dir.path())
            .args(["-b", "curl", "--add", command]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-b", "curl", "github"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL / CURL ==="))
        .stdout(predicate::str::contains("api.github.com/users"))
        .stdout(predicate::str::contains("api.github.com/repos"))
        .stdout(predicate::str::contains("example.com").not());
}

#[test]
fn test_import_multiline_curl_from_zsh_history() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();

    let zsh_history = home_dir.join(".zsh_history");
    let history_content = r#": 1647875000:0;echo "before"
: 1647875010:0;curl -X POST https://api.example.com/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ viewer { login } }"}'
: 1647875020:0;echo "after""#;

    fs::write(&zsh_history, history_content).unwrap();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir).arg("--import");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Imported 1 new curl commands"));

    let stored_commands =
        fs::read_to_string(home_dir.join(".reqbib").join("commands.json")).unwrap();
    assert!(stored_commands.contains("curl -X POST https://api.example.com/graphql\\n"));
    assert!(stored_commands.contains("  -H \\\"Content-Type: application/json\\\"\\n"));
    assert!(!stored_commands.contains("graphql \\\\\\\\"));

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir).arg("--list");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "curl -X POST https://api.example.com/graphql",
        ))
        .stdout(predicate::str::contains(
            "  -H \"Content-Type: application/json\"",
        ))
        .stdout(predicate::str::contains(
            "  -d '{\"query\":\"{ viewer { login } }\"}'",
        ))
        .stdout(predicate::str::contains("graphql \\").not())
        .stdout(predicate::str::contains("application/json\" \\").not());
}

#[test]
fn test_import_multiline_curl_from_zsh_history() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();

    let zsh_history = home_dir.join(".zsh_history");
    let history_content = r#": 1647875000:0;echo "before"
: 1647875010:0;curl -X POST https://api.example.com/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ viewer { login } }"}'
: 1647875020:0;echo "after""#;

    fs::write(&zsh_history, history_content).unwrap();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir).arg("--import");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Imported 1 new curl commands"));

    let stored_commands =
        fs::read_to_string(home_dir.join(".reqbib").join("commands.json")).unwrap();
    assert!(stored_commands.contains("curl -X POST https://api.example.com/graphql\\n"));
    assert!(stored_commands.contains("  -H \\\"Content-Type: application/json\\\"\\n"));
    assert!(!stored_commands.contains("graphql \\\\\\\\"));

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir).arg("--list");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "curl -X POST https://api.example.com/graphql",
        ))
        .stdout(predicate::str::contains(
            "  -H \"Content-Type: application/json\"",
        ))
        .stdout(predicate::str::contains(
            "  -d '{\"query\":\"{ viewer { login } }\"}'",
        ))
        .stdout(predicate::str::contains("graphql \\").not())
        .stdout(predicate::str::contains("application/json\" \\").not());
}

#[test]
fn test_duplicate_prevention() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("combib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["-b", "git", "--add", "git status"]);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-b", "git", "--add", "git status"]);
    cmd.assert().success().stdout(predicate::str::contains(
        "Command already exists in biblioteca 'git'.",
    ));

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-b", "git", "--list"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("git status"))
        .stdout(predicate::str::contains("[2]").not());
}

#[test]
fn test_add_command_to_team_repository() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-combib");
    let mut cmd = Command::cargo_bin("combib").unwrap();

    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "-b",
        "curl",
        "--add",
        "curl https://api.example.com/platform",
    ]);

    cmd.assert().success();

    let data_file = shared_repo
        .join("teams")
        .join("platform")
        .join("libs")
        .join("curl.json");
    assert!(data_file.exists());
}

#[test]
fn test_team_repository_storage_is_isolated_per_team() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-combib");

    for (team, command) in [
        ("platform", "curl https://api.example.com/platform"),
        ("payments", "curl https://api.example.com/payments"),
    ] {
        let mut cmd = Command::cargo_bin("combib").unwrap();
        cmd.env("HOME", temp_dir.path()).args([
            "--repo",
            shared_repo.to_str().unwrap(),
            "--team",
            team,
            "-b",
            "curl",
            "--add",
            command,
        ]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "-b",
        "curl",
        "--list",
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("api.example.com/platform"))
        .stdout(predicate::str::contains("api.example.com/payments").not());
}

#[test]
fn test_default_team_and_biblioteca_combined_read_hides_duplicates() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-combib");
    let combib_dir = home_dir.join(".combib");
    let config_path = combib_dir.join("config.json");

    fs::create_dir_all(&combib_dir).unwrap();
    write_path_config(
        &config_path,
        &shared_repo,
        "teams",
        Some("curl"),
        Some("platform"),
        None,
        None,
    );

    write_command_database(
        &home_dir.join(".combib").join("libs").join("curl.json"),
        &[
            (
                "curl https://shared.example.com/health",
                Some("Local shared copy"),
            ),
            ("curl https://local.example.com/health", Some("Local only")),
        ],
    );
    write_command_database(
        &shared_repo
            .join("teams")
            .join("platform")
            .join("libs")
            .join("curl.json"),
        &[(
            "curl https://shared.example.com/health",
            Some("Platform health"),
        )],
    );

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", home_dir).arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL / CURL ==="))
        .stdout(predicate::str::contains("=== SHARED / PLATFORM / CURL ==="))
        .stdout(predicate::str::contains(
            "curl https://local.example.com/health",
        ))
        .stdout(predicate::str::contains("[1] Platform health"))
        .stdout(predicate::str::contains(
            "1 local command was hidden because it duplicates shared storage.",
        ));
}

#[test]
fn test_shared_only_requires_default_shared_selection() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-combib");
    let combib_dir = home_dir.join(".combib");
    let config_path = combib_dir.join("config.json");

    fs::create_dir_all(&combib_dir).unwrap();
    write_path_config(
        &config_path,
        &shared_repo,
        "teams",
        Some("curl"),
        None,
        None,
        None,
    );

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", home_dir).args(["--shared-only", "--list"]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "No default shared selection configured",
    ));
}

#[test]
fn test_all_teams_list_uses_same_biblioteca() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-combib");

    write_command_database(
        &shared_repo
            .join("teams")
            .join("platform")
            .join("libs")
            .join("curl.json"),
        &[("curl https://platform.example.com/health", Some("Platform"))],
    );
    write_command_database(
        &shared_repo
            .join("teams")
            .join("payments")
            .join("libs")
            .join("curl.json"),
        &[("curl https://payments.example.com/health", Some("Payments"))],
    );

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--all-teams",
        "-b",
        "curl",
        "--list",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== SHARED / PAYMENTS / CURL ==="))
        .stdout(predicate::str::contains("=== SHARED / PLATFORM / CURL ==="))
        .stdout(predicate::str::contains("payments.example.com/health"))
        .stdout(predicate::str::contains("platform.example.com/health"));
}

#[test]
fn test_default_list_limit_applies() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let combib_dir = home_dir.join(".combib");
    let config_path = combib_dir.join("config.json");

    fs::create_dir_all(&combib_dir).unwrap();
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "default_biblioteca": "curl",
            "default_list_limit": 2
        }))
        .unwrap(),
    )
    .unwrap();

    for command in [
        "curl https://one.example.com",
        "curl https://two.example.com",
        "curl https://three.example.com",
    ] {
        let mut cmd = Command::cargo_bin("combib").unwrap();
        cmd.env("HOME", home_dir).args(["--add", command]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", home_dir).arg("--list");

    cmd.assert().success().stdout(predicate::str::contains(
        "Showing first 2 commands. 1 additional command was hidden by the active list limit.",
    ));
}

#[test]
fn test_github_mode_bootstraps_checkout_under_combib_directory() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let combib_dir = home_dir.join(".combib");
    let config_path = combib_dir.join("config.json");
    let (gh_path, gh_log_path) = write_mock_gh(home_dir);

    fs::create_dir_all(&combib_dir).unwrap();
    write_github_config(
        &config_path,
        "acme/shared-combib",
        "teams",
        GithubConfigOptions {
            default_biblioteca: Some("curl"),
            default_team: Some("platform"),
            auto_update_repo: Some(false),
            ..GithubConfigOptions::default()
        },
    );

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", home_dir)
        .env("COMBIB_GH_BIN", &gh_path)
        .args([
            "--team",
            "platform",
            "--add",
            "curl https://api.example.com/platform",
        ]);

    cmd.assert().success();

    let checkout_path = home_dir
        .join(".combib")
        .join("repos")
        .join("acme__shared-combib");
    assert!(checkout_path.exists());
    assert!(checkout_path
        .join("teams")
        .join("platform")
        .join("libs")
        .join("curl.json")
        .exists());

    let gh_args = fs::read_to_string(gh_log_path).unwrap();
    assert!(gh_args.contains("repo"));
    assert!(gh_args.contains("clone"));
    assert!(gh_args.contains("acme/shared-combib"));
}

#[test]
fn test_github_mode_updates_existing_checkout_when_due() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let combib_dir = home_dir.join(".combib");
    let config_path = combib_dir.join("config.json");
    let checkout_path = combib_dir.join("repos").join("acme__shared-combib");
    let (git_path, git_log_path) = write_mock_git(home_dir);

    fs::create_dir_all(checkout_path.join("teams").join("platform").join("libs")).unwrap();
    fs::create_dir_all(&combib_dir).unwrap();
    write_github_config(
        &config_path,
        "acme/shared-combib",
        "teams",
        GithubConfigOptions {
            default_biblioteca: Some("curl"),
            default_team: Some("platform"),
            auto_update_repo: Some(true),
            auto_update_interval_minutes: Some(15),
            ..GithubConfigOptions::default()
        },
    );

    let mut cmd = Command::cargo_bin("combib").unwrap();
    cmd.env("HOME", home_dir)
        .env("COMBIB_GIT_BIN", &git_path)
        .args(["--team", "platform", "--list"]);

    cmd.assert().success();

    let git_args = fs::read_to_string(git_log_path).unwrap();
    assert!(git_args.contains("-C"));
    assert!(git_args.contains("pull"));
    assert!(git_args.contains("--ff-only"));
}
