use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn write_path_config(config_path: &Path, shared_repo: &Path, teams_dir: &str) {
    let config = serde_json::json!({
        "shared_repo": {
            "mode": "path",
            "path": shared_repo,
            "teams_dir": teams_dir
        }
    });

    fs::write(config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
}

fn write_github_config(
    config_path: &Path,
    github_repo: &str,
    teams_dir: &str,
    auto_update_repo: Option<bool>,
    auto_update_interval_minutes: Option<u64>,
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
    if let Some(auto_update_repo) = auto_update_repo {
        shared_repo.insert(
            "auto_update_repo".to_string(),
            serde_json::Value::Bool(auto_update_repo),
        );
    }
    if let Some(auto_update_interval_minutes) = auto_update_interval_minutes {
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

#[test]
fn test_help_output() {
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "A CLI tool for managing curl commands",
        ))
        .stdout(predicate::str::contains("Usage: reqbib"))
        .stdout(predicate::str::contains("--add"))
        .stdout(predicate::str::contains("--import"))
        .stdout(predicate::str::contains("--list"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--repo"))
        .stdout(predicate::str::contains("--teams-dir"))
        .stdout(predicate::str::contains("--team"))
        .stdout(predicate::str::contains("--all-teams"))
        .stdout(predicate::str::contains("--local-only"))
        .stdout(predicate::str::contains("--shared-only"));
}

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicate::str::contains(
        "A CLI tool for managing curl commands",
    ));
}

#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));
}

#[test]
fn test_add_command() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    // Set HOME to temp directory to avoid affecting real data
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--add").arg("curl https://example.com/test");

    cmd.assert().success().stdout(predicate::str::contains(
        "Added curl command: curl https://example.com/test",
    ));

    // Verify the command was saved
    let data_file = temp_dir.path().join(".reqbib").join("commands.json");
    assert!(data_file.exists());

    let content = fs::read_to_string(data_file).unwrap();
    assert!(content.contains("curl https://example.com/test"));
}

#[test]
fn test_list_empty_database() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    cmd.env("HOME", temp_dir.path());
    cmd.arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No curl commands stored"));
}

#[test]
fn test_list_with_commands() {
    let temp_dir = TempDir::new().unwrap();

    // First, add a command
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--add").arg("curl https://example.com");
    cmd.assert().success();

    // Then list commands
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("[1]"))
        .stdout(predicate::str::contains("curl https://example.com"));
}

#[test]
fn test_search_commands() {
    let temp_dir = TempDir::new().unwrap();

    // Add multiple commands
    let commands = vec![
        "curl https://api.github.com/users",
        "curl https://example.com/test",
        "curl -X POST https://api.github.com/repos",
    ];

    for command in commands {
        let mut cmd = Command::cargo_bin("reqbib").unwrap();
        cmd.env("HOME", temp_dir.path());
        cmd.arg("--add").arg(command);
        cmd.assert().success();
    }

    // Search for github commands
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("github");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("api.github.com/users"))
        .stdout(predicate::str::contains("api.github.com/repos"));
}

#[test]
fn test_search_no_results() {
    let temp_dir = TempDir::new().unwrap();

    // Add a command
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--add").arg("curl https://example.com");
    cmd.assert().success();

    // Search for non-existent keyword
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("nonexistent");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No matching curl commands."));
}

#[test]
fn test_multiple_keyword_search() {
    let temp_dir = TempDir::new().unwrap();

    // Add commands
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--add")
        .arg("curl -X POST https://api.github.com/repos");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--add").arg("curl https://api.github.com/users");
    cmd.assert().success();

    // Search with multiple keywords
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.args(["github", "POST"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("api.github.com/repos"));
}

#[test]
fn test_import_from_mock_history() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();

    // Create mock bash history file
    let bash_history = home_dir.join(".bash_history");
    let history_content = r#"ls -la
curl https://example.com/api
cd /home/user
curl -X POST https://github.com/api/repos
git status
curl https://httpbin.org/get"#;

    fs::write(&bash_history, history_content).unwrap();

    // Run import command
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir);
    cmd.arg("--import");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Imported 3 new curl commands"));

    // Verify commands were imported by listing them
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir);
    cmd.arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("curl https://example.com/api"))
        .stdout(predicate::str::contains(
            "curl -X POST https://github.com/api/repos",
        ))
        .stdout(predicate::str::contains("curl https://httpbin.org/get"));
}

#[test]
fn test_import_from_zsh_history() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();

    // Create mock zsh history file
    let zsh_history = home_dir.join(".zsh_history");
    let history_content = r#": 1647875000:0;ls -la
: 1647875010:0;curl https://api.example.com
: 1647875020:0;cd /home/user
: 1647875030:0;curl -H "Content-Type: application/json" https://httpbin.org/post"#;

    fs::write(&zsh_history, history_content).unwrap();

    // Run import command
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir);
    cmd.arg("--import");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 new curl commands"));
}

#[test]
fn test_duplicate_prevention() {
    let temp_dir = TempDir::new().unwrap();

    // Add same command twice
    let curl_command = "curl https://example.com";

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--add").arg(curl_command);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--add").arg(curl_command);
    cmd.assert().success();

    // List should show only one command
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("[1]"));
}

#[test]
fn test_short_flags() {
    let temp_dir = TempDir::new().unwrap();

    // Test short add flag
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("-a").arg("curl https://example.com");
    cmd.assert().success();

    // Test short list flag
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("-l");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("[1]"));
}

#[test]
fn test_invalid_command_line_args() {
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.arg("--invalid-flag");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error: unexpected argument"));
}

#[test]
fn test_list_with_filter() {
    let temp_dir = TempDir::new().unwrap();

    // Add multiple commands
    let commands = vec![
        "curl https://api.github.com/users",
        "curl https://example.com/test",
        "curl -X POST https://api.github.com/repos",
        "curl https://gitlab.com/api/projects",
    ];

    for command in commands {
        let mut cmd = Command::cargo_bin("reqbib").unwrap();
        cmd.env("HOME", temp_dir.path());
        cmd.arg("--add").arg(command);
        cmd.assert().success();
    }

    // Test filtering with single keyword
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.args(["-l", "github"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("api.github.com/users"))
        .stdout(predicate::str::contains("api.github.com/repos"))
        .stdout(predicate::str::contains("example.com").not())
        .stdout(predicate::str::contains("gitlab.com").not());

    // Test filtering with multiple keywords
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.args(["-l", "github", "api"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("api.github.com/users"))
        .stdout(predicate::str::contains("api.github.com/repos"));

    // Test filtering with very specific keywords
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.args(["-l", "github", "POST"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("api.github.com/repos"))
        .stdout(predicate::str::contains("api.github.com/users").not());
}

#[test]
fn test_list_with_filter_no_results() {
    let temp_dir = TempDir::new().unwrap();

    // Add a command
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("--add").arg("curl https://example.com");
    cmd.assert().success();

    // Filter with non-matching keyword
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.args(["-l", "nonexistent"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No matching curl commands."));
}

#[test]
fn test_add_command_to_team_repository() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/platform");

    cmd.assert().success().stdout(predicate::str::contains(
        "Added curl command: curl https://api.example.com/platform",
    ));

    let data_file = shared_repo
        .join("teams")
        .join("platform")
        .join("commands.json");
    assert!(data_file.exists());

    let content = fs::read_to_string(data_file).unwrap();
    assert!(content.contains("curl https://api.example.com/platform"));
}

#[test]
fn test_team_repository_storage_is_isolated_per_team() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/platform");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("payments")
        .arg("--add")
        .arg("curl https://api.example.com/payments");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("platform")
        .arg("--list");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("api.example.com/platform"))
        .stdout(predicate::str::contains("api.example.com/payments").not());

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("payments")
        .arg("--list");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("api.example.com/payments"))
        .stdout(predicate::str::contains("api.example.com/platform").not());
}

#[test]
fn test_invalid_team_name_fails() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("../platform")
        .arg("--list");

    cmd.assert().failure().stderr(predicate::str::contains(
        "Team names may only contain letters, numbers, dots, underscores, and hyphens.",
    ));
}

#[test]
fn test_team_repository_uses_default_config() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-reqbib");
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");

    fs::create_dir_all(&reqbib_dir).unwrap();
    write_path_config(&config_path, &shared_repo, "company-teams");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir);
    cmd.arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/platform");

    cmd.assert().success();

    let data_file = shared_repo
        .join("company-teams")
        .join("platform")
        .join("commands.json");
    assert!(data_file.exists());
}

#[test]
fn test_cli_repo_overrides_configured_repo() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let configured_repo = home_dir.join("configured-repo");
    let override_repo = home_dir.join("override-repo");
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");

    fs::create_dir_all(&reqbib_dir).unwrap();
    write_path_config(&config_path, &configured_repo, "company-teams");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir);
    cmd.arg("--repo")
        .arg(&override_repo)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/platform");

    cmd.assert().success();

    assert!(override_repo
        .join("company-teams")
        .join("platform")
        .join("commands.json")
        .exists());
    assert!(!configured_repo
        .join("company-teams")
        .join("platform")
        .join("commands.json")
        .exists());
}

#[test]
fn test_cli_teams_dir_overrides_configured_teams_dir() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-reqbib");
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");

    fs::create_dir_all(&reqbib_dir).unwrap();
    write_path_config(&config_path, &shared_repo, "company-teams");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir);
    cmd.arg("--team")
        .arg("platform")
        .arg("--teams-dir")
        .arg("custom-teams")
        .arg("--add")
        .arg("curl https://api.example.com/platform");

    cmd.assert().success();

    assert!(shared_repo
        .join("custom-teams")
        .join("platform")
        .join("commands.json")
        .exists());
    assert!(!shared_repo
        .join("company-teams")
        .join("platform")
        .join("commands.json")
        .exists());
}

#[test]
fn test_default_reads_combine_local_and_shared_when_shared_repo_is_configured() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-reqbib");
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");

    fs::create_dir_all(&reqbib_dir).unwrap();
    write_path_config(&config_path, &shared_repo, "company-teams");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--add")
        .arg("curl https://local.example.com/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://shared.example.com/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir).arg("health");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains("Shared / platform"))
        .stdout(predicate::str::contains(
            "curl https://local.example.com/health",
        ))
        .stdout(predicate::str::contains(
            "curl https://shared.example.com/health",
        ));
}

#[test]
fn test_local_only_limits_default_reads_to_local_commands() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-reqbib");
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");

    fs::create_dir_all(&reqbib_dir).unwrap();
    write_path_config(&config_path, &shared_repo, "company-teams");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--add")
        .arg("curl https://local.example.com/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://shared.example.com/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir).arg("--local-only").arg("health");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local"))
        .stdout(predicate::str::contains(
            "curl https://local.example.com/health",
        ))
        .stdout(predicate::str::contains("Shared / platform").not())
        .stdout(predicate::str::contains("curl https://shared.example.com/health").not());
}

#[test]
fn test_shared_only_limits_default_reads_to_shared_commands() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-reqbib");
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");

    fs::create_dir_all(&reqbib_dir).unwrap();
    write_path_config(&config_path, &shared_repo, "company-teams");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--add")
        .arg("curl https://local.example.com/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://shared.example.com/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir).arg("--shared-only").arg("health");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Shared / platform"))
        .stdout(predicate::str::contains(
            "curl https://shared.example.com/health",
        ))
        .stdout(predicate::str::contains("Local").not())
        .stdout(predicate::str::contains("curl https://local.example.com/health").not());
}

#[test]
fn test_config_default_read_scope_shared_limits_default_reads() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-reqbib");
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");

    fs::create_dir_all(&reqbib_dir).unwrap();
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "shared_repo": {
                "mode": "path",
                "path": shared_repo,
                "teams_dir": "company-teams"
            },
            "default_read_scope": "shared"
        }))
        .unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--add")
        .arg("curl https://local.example.com/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://shared.example.com/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir).arg("health");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Shared / platform"))
        .stdout(predicate::str::contains(
            "curl https://shared.example.com/health",
        ))
        .stdout(predicate::str::contains("Local").not())
        .stdout(predicate::str::contains("curl https://local.example.com/health").not());
}

#[test]
fn test_repo_without_team_reads_from_shared_scope() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/platform");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Shared / platform"))
        .stdout(predicate::str::contains(
            "curl https://api.example.com/platform",
        ));
}

#[test]
fn test_team_without_repo_or_config_fails() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--team")
        .arg("platform")
        .arg("--list");

    cmd.assert().failure().stderr(predicate::str::contains(
        "No shared repository configured. Use --repo or configure shared_repo in config.",
    ));
}

#[test]
fn test_legacy_flat_config_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");

    fs::create_dir_all(&reqbib_dir).unwrap();
    fs::write(
        &config_path,
        r#"{
  "github_repo": "acme/shared-reqbib",
  "teams_dir": "company-teams"
}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .arg("--team")
        .arg("platform")
        .arg("--list");

    cmd.assert().failure().stderr(predicate::str::contains(
        "Legacy flat shared repository config is no longer supported.",
    ));
}

#[test]
fn test_team_repository_uses_github_repo_config_with_gh_clone() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");
    let (gh_path, gh_log) = write_mock_gh(home_dir);

    fs::create_dir_all(&reqbib_dir).unwrap();
    write_github_config(
        &config_path,
        "acme/shared-reqbib",
        "company-teams",
        None,
        None,
    );

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .env("REQBIB_GH_BIN", &gh_path)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/platform");

    cmd.assert().success();

    let checkout_root = home_dir
        .join(".reqbib")
        .join("repos")
        .join("acme__shared-reqbib");
    let data_file = checkout_root
        .join("company-teams")
        .join("platform")
        .join("commands.json");
    assert!(data_file.exists());

    let gh_args = fs::read_to_string(gh_log).unwrap();
    assert!(gh_args.contains("repo"));
    assert!(gh_args.contains("clone"));
    assert!(gh_args.contains("acme/shared-reqbib"));
}

#[test]
fn test_existing_github_checkout_skips_gh_clone() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");
    let checkout_root = home_dir
        .join(".reqbib")
        .join("repos")
        .join("acme__shared-reqbib");
    let gh_log = home_dir.join("gh.log");

    fs::create_dir_all(&reqbib_dir).unwrap();
    fs::create_dir_all(&checkout_root).unwrap();
    write_github_config(
        &config_path,
        "acme/shared-reqbib",
        "company-teams",
        None,
        None,
    );

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .env("REQBIB_GH_BIN", home_dir.join("missing-gh"))
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/platform");

    cmd.assert().success();

    assert!(checkout_root
        .join("company-teams")
        .join("platform")
        .join("commands.json")
        .exists());
    assert!(!gh_log.exists());
}

#[test]
fn test_existing_github_checkout_auto_updates_with_git() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");
    let checkout_root = home_dir
        .join(".reqbib")
        .join("repos")
        .join("acme__shared-reqbib");
    let company_teams = checkout_root.join("company-teams");
    let (git_path, git_log) = write_mock_git(home_dir);

    fs::create_dir_all(&reqbib_dir).unwrap();
    fs::create_dir_all(&company_teams).unwrap();
    write_github_config(
        &config_path,
        "acme/shared-reqbib",
        "company-teams",
        Some(true),
        Some(60),
    );

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .env("REQBIB_GIT_BIN", &git_path)
        .arg("--team")
        .arg("platform")
        .arg("--list");

    cmd.assert().success();

    let git_args = fs::read_to_string(git_log).unwrap();
    assert!(git_args.contains("pull"));
    assert!(git_args.contains("--ff-only"));
}

#[test]
fn test_existing_github_checkout_can_disable_auto_update() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let reqbib_dir = home_dir.join(".reqbib");
    let config_path = reqbib_dir.join("config.json");
    let checkout_root = home_dir
        .join(".reqbib")
        .join("repos")
        .join("acme__shared-reqbib");
    let company_teams = checkout_root.join("company-teams");
    let git_log = home_dir.join("git.log");

    fs::create_dir_all(&reqbib_dir).unwrap();
    fs::create_dir_all(&company_teams).unwrap();
    write_github_config(
        &config_path,
        "acme/shared-reqbib",
        "company-teams",
        Some(false),
        None,
    );

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", home_dir)
        .env("REQBIB_GIT_BIN", home_dir.join("missing-git"))
        .arg("--team")
        .arg("platform")
        .arg("--list");

    cmd.assert().success();

    assert!(!git_log.exists());
}

#[test]
fn test_list_across_all_teams() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/platform");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("payments")
        .arg("--add")
        .arg("curl https://api.example.com/payments");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--all-teams")
        .arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Shared / platform"))
        .stdout(predicate::str::contains("Shared / payments"))
        .stdout(predicate::str::contains(
            "curl https://api.example.com/platform",
        ))
        .stdout(predicate::str::contains(
            "curl https://api.example.com/payments",
        ));
}

#[test]
fn test_search_across_all_teams() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("platform")
        .arg("--add")
        .arg("curl https://api.example.com/shared/health");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("payments")
        .arg("--add")
        .arg("curl https://api.example.com/shared/webhook");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--all-teams")
        .arg("shared");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Shared / platform"))
        .stdout(predicate::str::contains("Shared / payments"))
        .stdout(predicate::str::contains(
            "curl https://api.example.com/shared/health",
        ))
        .stdout(predicate::str::contains(
            "curl https://api.example.com/shared/webhook",
        ));
}

#[test]
fn test_team_and_all_teams_conflict() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("platform")
        .arg("--all-teams")
        .arg("--list");

    cmd.assert().failure().stderr(predicate::str::contains(
        "--team cannot be used together with --all-teams.",
    ));
}

#[test]
fn test_add_with_all_teams_fails() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--all-teams")
        .arg("--add")
        .arg("curl https://api.example.com/platform");

    cmd.assert().failure().stderr(predicate::str::contains(
        "--all-teams cannot be used with --add.",
    ));
}

#[test]
fn test_local_only_with_team_fails() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-reqbib");
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--repo")
        .arg(&shared_repo)
        .arg("--team")
        .arg("platform")
        .arg("--local-only")
        .arg("--list");

    cmd.assert().failure().stderr(predicate::str::contains(
        "--local-only and --shared-only cannot be used with --team.",
    ));
}

#[test]
fn test_shared_only_with_add_fails() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("reqbib").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--shared-only")
        .arg("--add")
        .arg("curl https://example.com");

    cmd.assert().failure().stderr(predicate::str::contains(
        "--local-only and --shared-only cannot be used with --add.",
    ));
}
