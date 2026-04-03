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
    default_shelf: Option<&str>,
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
    if let Some(default_shelf) = default_shelf {
        config.insert(
            "default_shelf".to_string(),
            serde_json::Value::String(default_shelf.to_string()),
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
    default_shelf: Option<&'a str>,
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
    if let Some(default_shelf) = options.default_shelf {
        config.insert(
            "default_shelf".to_string(),
            serde_json::Value::String(default_shelf.to_string()),
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

fn write_publish_mock_git(temp_dir: &Path) -> (PathBuf, PathBuf) {
    let log_path = temp_dir.join("publish-git.log");
    let git_path = if cfg!(windows) {
        temp_dir.join("publish-git.cmd")
    } else {
        temp_dir.join("publish-git")
    };

    let script = if cfg!(windows) {
        format!(
            "@echo off\r\n\
setlocal EnableDelayedExpansion\r\n\
echo %*>> \"{}\"\r\n\
if \"%1\"==\"status\" (\r\n\
  if \"%3\"==\"--\" (\r\n\
    echo M %4\r\n\
    exit /b 0\r\n\
  )\r\n\
  if not \"%SHELLSHELF_TEST_GIT_STATUS%\"==\"\" <nul set /p =%SHELLSHELF_TEST_GIT_STATUS%\r\n\
  exit /b 0\r\n\
)\r\n\
if \"%1\"==\"symbolic-ref\" (\r\n\
  echo %SHELLSHELF_TEST_GIT_REMOTE_HEAD%\r\n\
  exit /b 0\r\n\
)\r\n\
if \"%1\"==\"branch\" (\r\n\
  echo %SHELLSHELF_TEST_GIT_CURRENT_BRANCH%\r\n\
  exit /b 0\r\n\
)\r\n\
if \"%1\"==\"show-ref\" (\r\n\
  set REF=%4\r\n\
  if /I \"!REF:~0,11!\"==\"refs/heads/\" (\r\n\
    set BRANCH=!REF:~11!\r\n\
    echo ,%SHELLSHELF_TEST_GIT_LOCAL_BRANCHES%, | findstr /C:\",!BRANCH!,\" >nul && exit /b 0\r\n\
    exit /b 1\r\n\
  )\r\n\
  if /I \"!REF:~0,20!\"==\"refs/remotes/origin/\" (\r\n\
    set BRANCH=!REF:~20!\r\n\
    echo ,%SHELLSHELF_TEST_GIT_REMOTE_BRANCHES%, | findstr /C:\",!BRANCH!,\" >nul && exit /b 0\r\n\
    exit /b 1\r\n\
  )\r\n\
)\r\n\
exit /b 0\r\n",
            log_path.display()
        )
    } else {
        format!(
            "#!/bin/sh\n\
printf '%s\\n' \"$*\" >> \"{}\"\n\
case \"$1\" in\n\
  status)\n\
    if [ \"$3\" = \"--\" ]; then\n\
      printf ' M %s' \"$4\"\n\
      exit 0\n\
    fi\n\
    if [ -n \"$SHELLSHELF_TEST_GIT_STATUS\" ]; then\n\
      printf '%s' \"$SHELLSHELF_TEST_GIT_STATUS\"\n\
    fi\n\
    exit 0\n\
    ;;\n\
  symbolic-ref)\n\
    printf '%s\\n' \"${{SHELLSHELF_TEST_GIT_REMOTE_HEAD:-refs/remotes/origin/main}}\"\n\
    exit 0\n\
    ;;\n\
  branch)\n\
    printf '%s\\n' \"${{SHELLSHELF_TEST_GIT_CURRENT_BRANCH:-main}}\"\n\
    exit 0\n\
    ;;\n\
  show-ref)\n\
    ref=\"$4\"\n\
    case \"$ref\" in\n\
      refs/heads/*)\n\
        branch=\"${{ref#refs/heads/}}\"\n\
        case \",${{SHELLSHELF_TEST_GIT_LOCAL_BRANCHES}},\" in\n\
          *,\"$branch\",*) exit 0 ;;\n\
          *) exit 1 ;;\n\
        esac\n\
        ;;\n\
      refs/remotes/origin/*)\n\
        branch=\"${{ref#refs/remotes/origin/}}\"\n\
        case \",${{SHELLSHELF_TEST_GIT_REMOTE_BRANCHES:-main}},\" in\n\
          *,\"$branch\",*) exit 0 ;;\n\
          *) exit 1 ;;\n\
        esac\n\
        ;;\n\
    esac\n\
    ;;\n\
esac\n\
exit 0\n",
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

fn write_publish_mock_gh(temp_dir: &Path) -> (PathBuf, PathBuf) {
    let log_path = temp_dir.join("publish-gh.log");
    let gh_path = if cfg!(windows) {
        temp_dir.join("publish-gh.cmd")
    } else {
        temp_dir.join("publish-gh")
    };

    let script = if cfg!(windows) {
        format!(
            "@echo off\r\n\
setlocal\r\n\
echo %*>> \"{}\"\r\n\
if \"%1\"==\"pr\" echo %SHELLSHELF_TEST_GH_PR_URL%\r\n",
            log_path.display()
        )
    } else {
        format!(
            "#!/bin/sh\n\
printf '%s\\n' \"$*\" >> \"{}\"\n\
if [ \"$1\" = \"pr\" ]; then\n\
  printf '%s\\n' \"${{SHELLSHELF_TEST_GH_PR_URL:-https://github.com/acme/shared-shellshelf/pull/1}}\"\n\
fi\n",
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

fn write_text_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[test]
fn test_help_output() {
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "A CLI for storing, searching, and sharing reusable shell commands",
        ))
        .stdout(predicate::str::contains("Usage: shellshelf"))
        .stdout(predicate::str::contains("--shelf"))
        .stdout(predicate::str::contains("--list-shelves"))
        .stdout(predicate::str::contains("--list"))
        .stdout(predicate::str::contains("--import-postman"))
        .stdout(predicate::str::contains("--web"))
        .stdout(predicate::str::contains("--web-port"));
}

#[test]
fn test_add_uses_builtin_default_shelf_without_config() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path())
        .arg("--add")
        .arg("curl https://example.com");

    cmd.assert().success().stdout(predicate::str::contains(
        "Added command to shelf 'default': curl https://example.com",
    ));

    assert!(temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("default.json")
        .exists());
}

#[test]
fn test_web_port_requires_web_mode() {
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.arg("--web-port").arg("4812");

    cmd.assert().failure().stderr(predicate::str::contains(
        "--web-port can only be used with --web.",
    ));
}

#[test]
fn test_web_mode_rejects_standard_cli_flags() {
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.args(["--web", "--list"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains(
            "--web cannot be combined with --add, --list, --list-shelves, --create-shelf, or --import-postman.",
        ));
}

#[test]
fn test_add_command_local() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["-s", "curl", "--add", "curl https://example.com/test"]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Added command to shelf 'curl': curl https://example.com/test",
    ));

    let data_file = temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("curl.json");
    assert!(data_file.exists());

    let content = fs::read_to_string(data_file).unwrap();
    assert!(content.contains("curl https://example.com/test"));
}

#[test]
fn test_create_local_shelf() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["--create-shelf", "git"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Created shelf 'git'."));

    assert!(temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("git.json")
        .exists());
}

#[test]
fn test_create_shelf_in_team_repository() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "--create-shelf",
        "aws",
    ]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Created shelf 'aws' for team 'platform'.",
    ));

    assert!(shared_repo
        .join("teams")
        .join("platform")
        .join("shelves")
        .join("aws.json")
        .exists());
}

#[test]
fn test_create_shelf_rejects_mismatched_active_shelf() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["-s", "curl", "--create-shelf", "git"]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "--shelf must match --create-shelf when both are provided.",
    ));
}

#[test]
fn test_list_local_shelves() {
    let temp_dir = TempDir::new().unwrap();

    for shelf in ["curl", "git"] {
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", temp_dir.path())
            .args(["--create-shelf", shelf]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).arg("--list-shelves");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL ==="))
        .stdout(predicate::str::contains("[1] curl"))
        .stdout(predicate::str::contains("[2] git"));
}

#[test]
fn test_list_team_shelves() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");

    for shelf in ["aws", "curl"] {
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", temp_dir.path()).args([
            "--repo",
            shared_repo.to_str().unwrap(),
            "--team",
            "platform",
            "--create-shelf",
            shelf,
        ]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "--list-shelves",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== SHARED / PLATFORM ==="))
        .stdout(predicate::str::contains("[1] aws"))
        .stdout(predicate::str::contains("[2] curl"));
}

#[test]
fn test_list_all_team_shelves() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");

    for (team, shelf) in [("payments", "curl"), ("platform", "aws")] {
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", temp_dir.path()).args([
            "--repo",
            shared_repo.to_str().unwrap(),
            "--team",
            team,
            "--create-shelf",
            shelf,
        ]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--all-teams",
        "--list-shelves",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== SHARED / PAYMENTS ==="))
        .stdout(predicate::str::contains("=== SHARED / PLATFORM ==="))
        .stdout(predicate::str::contains("[1] curl"))
        .stdout(predicate::str::contains("[1] aws"));
}

#[test]
fn test_list_shelves_rejects_shelf_flag() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["-s", "curl", "--list-shelves"]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "--shelf cannot be used with --list-shelves.",
    ));
}

#[test]
fn test_add_command_with_description() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path()).args([
        "-s",
        "curl",
        "--add",
        "curl https://example.com/test",
        "--description",
        "Example request",
    ]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Added command to shelf 'curl': curl https://example.com/test (Example request)",
    ));
}

#[test]
fn test_list_empty_shelf() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["-s", "curl", "--list"]);

    cmd.assert().success().stdout(predicate::str::contains(
        "No commands stored in shelf 'curl'.",
    ));
}

#[test]
fn test_list_with_commands() {
    let temp_dir = TempDir::new().unwrap();
    let multiline_command = "curl -X POST https://api.example.com/graphql \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\"query\":\"{ viewer { login } }\"}'";

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-s", "curl", "--add", multiline_command]);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-s", "curl", "--list"]);

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
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", temp_dir.path())
            .args(["-s", "curl", "--add", command]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-s", "curl", "github"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL / CURL ==="))
        .stdout(predicate::str::contains("api.github.com/users"))
        .stdout(predicate::str::contains("api.github.com/repos"))
        .stdout(predicate::str::contains("example.com").not());
}

#[test]
fn test_search_without_shelf_scans_all_local_shelves() {
    let temp_dir = TempDir::new().unwrap();

    for (shelf, command) in [
        ("curl", "curl https://api.github.com/users"),
        ("git", "git status"),
    ] {
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", temp_dir.path())
            .args(["-s", shelf, "--add", command]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL / GIT ==="))
        .stdout(predicate::str::contains("git status"))
        .stdout(predicate::str::contains("=== LOCAL / CURL ===").not());
}

#[test]
fn test_search_without_shelf_matches_shelf_name_and_command_term() {
    let temp_dir = TempDir::new().unwrap();

    for (shelf, command) in [
        ("media", "curl https://example.com/upload"),
        ("git", "git status"),
    ] {
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", temp_dir.path())
            .args(["-s", shelf, "--add", command]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args(["media", "upload"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL / MEDIA ==="))
        .stdout(predicate::str::contains("curl https://example.com/upload"))
        .stdout(predicate::str::contains("=== LOCAL / GIT ===").not());
}

#[test]
fn test_search_with_active_shelf_matches_repeated_shelf_name_keyword() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "-s",
        "media",
        "--add",
        "curl https://example.com/upload",
    ]);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-s", "media", "media", "upload"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL / MEDIA ==="))
        .stdout(predicate::str::contains("curl https://example.com/upload"));
}

#[test]
fn test_team_search_without_shelf_scans_all_team_shelves() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");

    for (shelf, command) in [
        ("curl", "curl https://api.example.com/platform"),
        ("git", "git deploy platform"),
    ] {
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", temp_dir.path()).args([
            "--repo",
            shared_repo.to_str().unwrap(),
            "--team",
            "platform",
            "-s",
            shelf,
            "--add",
            command,
        ]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "deploy",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== SHARED / PLATFORM / GIT ==="))
        .stdout(predicate::str::contains("git deploy platform"))
        .stdout(predicate::str::contains("=== SHARED / PLATFORM / CURL ===").not());
}

#[test]
fn test_team_search_without_shelf_matches_shelf_name_only() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");

    write_command_database(
        &shared_repo
            .join("teams")
            .join("platform")
            .join("shelves")
            .join("media.json"),
        &[("curl https://example.com/upload", Some("Upload asset"))],
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "media",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "=== SHARED / PLATFORM / MEDIA ===",
        ))
        .stdout(predicate::str::contains("curl https://example.com/upload"));
}

#[test]
fn test_all_teams_search_without_shelf_matches_shelf_name_only() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");

    write_command_database(
        &shared_repo
            .join("teams")
            .join("platform")
            .join("shelves")
            .join("media.json"),
        &[(
            "curl https://platform.example.com/upload",
            Some("Platform upload"),
        )],
    );
    write_command_database(
        &shared_repo
            .join("teams")
            .join("payments")
            .join("shelves")
            .join("media.json"),
        &[(
            "curl https://payments.example.com/upload",
            Some("Payments upload"),
        )],
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--all-teams",
        "media",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "=== SHARED / PAYMENTS / MEDIA ===",
        ))
        .stdout(predicate::str::contains(
            "=== SHARED / PLATFORM / MEDIA ===",
        ))
        .stdout(predicate::str::contains("platform.example.com/upload"))
        .stdout(predicate::str::contains("payments.example.com/upload"));
}

#[test]
fn test_default_combined_search_hides_local_duplicate_when_match_comes_from_shelf_name() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-shellshelf");
    let shellshelf_dir = home_dir.join(".shellshelf");
    let config_path = shellshelf_dir.join("config.json");

    fs::create_dir_all(&shellshelf_dir).unwrap();
    write_path_config(
        &config_path,
        &shared_repo,
        "teams",
        None,
        Some("platform"),
        None,
        None,
    );

    let shared_command = "curl https://cdn.example.com/upload";
    write_command_database(
        &home_dir
            .join(".shellshelf")
            .join("shelves")
            .join("media.json"),
        &[(shared_command, Some("Local shared copy"))],
    );
    write_command_database(
        &shared_repo
            .join("teams")
            .join("platform")
            .join("shelves")
            .join("media.json"),
        &[
            (shared_command, Some("Platform upload")),
            (
                "curl https://cdn.example.com/local-only",
                Some("Shared only"),
            ),
        ],
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", home_dir).args(["media", "upload"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== LOCAL / MEDIA ===").not())
        .stdout(predicate::str::contains(
            "=== SHARED / PLATFORM / MEDIA ===",
        ))
        .stdout(predicate::str::contains("[1] Platform upload"))
        .stdout(predicate::str::contains(
            "1 local command was hidden because it duplicates shared storage.",
        ));
}

#[test]
fn test_duplicate_prevention() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["-s", "git", "--add", "git status"]);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-s", "git", "--add", "git status"]);
    cmd.assert().success().stdout(predicate::str::contains(
        "Command already exists in shelf 'git'.",
    ));

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["-s", "git", "--list"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("git status"))
        .stdout(predicate::str::contains("[2]").not());
}

#[test]
fn test_add_command_to_team_repository() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "-s",
        "curl",
        "--add",
        "curl https://api.example.com/platform",
    ]);

    cmd.assert().success();

    let data_file = shared_repo
        .join("teams")
        .join("platform")
        .join("shelves")
        .join("curl.json");
    assert!(data_file.exists());
}

#[test]
fn test_open_pr_publishes_shared_add_on_clean_branch() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");
    let (git_path, git_log_path) = write_publish_mock_git(temp_dir.path());
    let (gh_path, gh_log_path) = write_publish_mock_gh(temp_dir.path());

    fs::create_dir_all(&shared_repo).unwrap();

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .env("SHELLSHELF_GIT_BIN", &git_path)
        .env("SHELLSHELF_GH_BIN", &gh_path)
        .env("SHELLSHELF_TEST_GIT_CURRENT_BRANCH", "main")
        .env("SHELLSHELF_TEST_GIT_REMOTE_BRANCHES", "main,trunk")
        .env(
            "SHELLSHELF_TEST_GH_PR_URL",
            "https://github.com/acme/shared-shellshelf/pull/42",
        )
        .args([
            "--repo",
            shared_repo.to_str().unwrap(),
            "--team",
            "platform",
            "-s",
            "curl",
            "--add",
            "curl https://api.example.com/platform",
            "--open-pr",
            "--base-branch",
            "trunk",
            "--pr-branch",
            "feat/platform-curl",
        ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Added command to shelf 'curl': curl https://api.example.com/platform",
        ))
        .stdout(predicate::str::contains(
            "Opened pull request: https://github.com/acme/shared-shellshelf/pull/42",
        ));

    let data_file = shared_repo
        .join("teams")
        .join("platform")
        .join("shelves")
        .join("curl.json");
    assert!(data_file.exists());

    let git_log = fs::read_to_string(git_log_path).unwrap();
    assert!(git_log.contains("status --porcelain"));
    assert!(git_log.contains("fetch origin trunk"));
    assert!(git_log.contains("switch -c feat/platform-curl"));
    assert!(git_log.contains("rebase origin/trunk"));
    assert!(git_log.contains("add"));
    assert!(git_log.contains("commit -m Update platform/curl shelf"));
    assert!(git_log.contains("push --set-upstream origin feat/platform-curl"));

    let gh_log = fs::read_to_string(gh_log_path).unwrap();
    assert!(gh_log.contains("pr create"));
    assert!(gh_log.contains("--base trunk"));
    assert!(gh_log.contains("--head feat/platform-curl"));
    assert!(gh_log.contains("--title Update platform/curl shelf"));
}

#[test]
fn test_open_pr_rejects_dirty_shared_checkout_before_writing() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");
    let (git_path, _) = write_publish_mock_git(temp_dir.path());

    fs::create_dir_all(&shared_repo).unwrap();

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .env("SHELLSHELF_GIT_BIN", &git_path)
        .env(
            "SHELLSHELF_TEST_GIT_STATUS",
            " M teams/platform/shelves/curl.json",
        )
        .args([
            "--repo",
            shared_repo.to_str().unwrap(),
            "--team",
            "platform",
            "-s",
            "curl",
            "--add",
            "curl https://api.example.com/platform",
            "--open-pr",
        ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "Shared repository checkout has uncommitted changes.",
    ));

    assert!(!shared_repo
        .join("teams")
        .join("platform")
        .join("shelves")
        .join("curl.json")
        .exists());
}

#[test]
fn test_team_repository_storage_is_isolated_per_team() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");

    for (team, command) in [
        ("platform", "curl https://api.example.com/platform"),
        ("payments", "curl https://api.example.com/payments"),
    ] {
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", temp_dir.path()).args([
            "--repo",
            shared_repo.to_str().unwrap(),
            "--team",
            team,
            "-s",
            "curl",
            "--add",
            command,
        ]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "-s",
        "curl",
        "--list",
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("api.example.com/platform"))
        .stdout(predicate::str::contains("api.example.com/payments").not());
}

#[test]
fn test_default_team_and_shelf_combined_read_hides_duplicates() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-shellshelf");
    let shellshelf_dir = home_dir.join(".shellshelf");
    let config_path = shellshelf_dir.join("config.json");

    fs::create_dir_all(&shellshelf_dir).unwrap();
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
        &home_dir
            .join(".shellshelf")
            .join("shelves")
            .join("curl.json"),
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
            .join("shelves")
            .join("curl.json"),
        &[(
            "curl https://shared.example.com/health",
            Some("Platform health"),
        )],
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
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
fn test_shared_only_defaults_to_all_teams_when_shared_repo_is_configured() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shared_repo = home_dir.join("shared-shellshelf");
    let shellshelf_dir = home_dir.join(".shellshelf");
    let config_path = shellshelf_dir.join("config.json");

    fs::create_dir_all(&shellshelf_dir).unwrap();
    write_path_config(
        &config_path,
        &shared_repo,
        "teams",
        Some("curl"),
        None,
        None,
        None,
    );

    write_command_database(
        &shared_repo
            .join("teams")
            .join("platform")
            .join("shelves")
            .join("curl.json"),
        &[("curl https://platform.example.com/health", Some("Platform"))],
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", home_dir).args(["--shared-only", "--list"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== SHARED / PLATFORM / CURL ==="))
        .stdout(predicate::str::contains("platform.example.com/health"));
}

#[test]
fn test_all_teams_list_uses_same_shelf() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");

    write_command_database(
        &shared_repo
            .join("teams")
            .join("platform")
            .join("shelves")
            .join("curl.json"),
        &[("curl https://platform.example.com/health", Some("Platform"))],
    );
    write_command_database(
        &shared_repo
            .join("teams")
            .join("payments")
            .join("shelves")
            .join("curl.json"),
        &[("curl https://payments.example.com/health", Some("Payments"))],
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--all-teams",
        "-s",
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
    let shellshelf_dir = home_dir.join(".shellshelf");
    let config_path = shellshelf_dir.join("config.json");

    fs::create_dir_all(&shellshelf_dir).unwrap();
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "default_shelf": "curl",
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
        let mut cmd = Command::cargo_bin("shellshelf").unwrap();
        cmd.env("HOME", home_dir).args(["--add", command]);
        cmd.assert().success();
    }

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", home_dir).arg("--list");

    cmd.assert().success().stdout(predicate::str::contains(
        "Showing first 2 commands. 1 additional command was hidden by the active list limit.",
    ));
}

#[test]
fn test_github_mode_bootstraps_checkout_under_shellshelf_directory() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shellshelf_dir = home_dir.join(".shellshelf");
    let config_path = shellshelf_dir.join("config.json");
    let (gh_path, gh_log_path) = write_mock_gh(home_dir);

    fs::create_dir_all(&shellshelf_dir).unwrap();
    write_github_config(
        &config_path,
        "acme/shared-shellshelf",
        "teams",
        GithubConfigOptions {
            default_shelf: Some("curl"),
            default_team: Some("platform"),
            auto_update_repo: Some(false),
            ..GithubConfigOptions::default()
        },
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", home_dir)
        .env("SHELLSHELF_GH_BIN", &gh_path)
        .args([
            "--team",
            "platform",
            "--add",
            "curl https://api.example.com/platform",
        ]);

    cmd.assert().success();

    let checkout_path = home_dir
        .join(".shellshelf")
        .join("repos")
        .join("acme__shared-shellshelf");
    assert!(checkout_path.exists());
    assert!(checkout_path
        .join("teams")
        .join("platform")
        .join("shelves")
        .join("curl.json")
        .exists());

    let gh_args = fs::read_to_string(gh_log_path).unwrap();
    assert!(gh_args.contains("repo"));
    assert!(gh_args.contains("clone"));
    assert!(gh_args.contains("acme/shared-shellshelf"));
}

#[test]
fn test_github_mode_updates_existing_checkout_when_due() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shellshelf_dir = home_dir.join(".shellshelf");
    let config_path = shellshelf_dir.join("config.json");
    let checkout_path = shellshelf_dir.join("repos").join("acme__shared-shellshelf");
    let (git_path, git_log_path) = write_mock_git(home_dir);

    fs::create_dir_all(checkout_path.join("teams").join("platform").join("shelves")).unwrap();
    fs::create_dir_all(&shellshelf_dir).unwrap();
    write_github_config(
        &config_path,
        "acme/shared-shellshelf",
        "teams",
        GithubConfigOptions {
            default_shelf: Some("curl"),
            default_team: Some("platform"),
            auto_update_repo: Some(true),
            auto_update_interval_minutes: Some(15),
            ..GithubConfigOptions::default()
        },
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", home_dir)
        .env("SHELLSHELF_GIT_BIN", &git_path)
        .args(["--team", "platform", "--list"]);

    cmd.assert().success();

    let git_args = fs::read_to_string(git_log_path).unwrap();
    assert!(git_args.contains("-C"));
    assert!(git_args.contains("pull"));
    assert!(git_args.contains("--ff-only"));
}

#[test]
fn test_add_repo_writes_github_shared_repo_config_from_url() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shellshelf_dir = home_dir.join(".shellshelf");
    let config_path = shellshelf_dir.join("config.json");

    fs::create_dir_all(&shellshelf_dir).unwrap();
    write_text_file(
        &config_path,
        r#"{
  "default_shelf": "curl",
  "web": {
    "port": 4920,
    "theme": "giphy"
  },
  "shared_repo": {
    "mode": "path",
    "path": "/tmp/old-shared-shellshelf",
    "teams_dir": "company-teams",
    "default_team": "platform"
  }
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", home_dir).args([
        "--add-repo",
        "https://github.com/acme/shared-shellshelf.git",
    ]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Configured shared GitHub repository 'acme/shared-shellshelf'",
    ));

    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "default_shelf": "curl",
            "shared_repo": {
                "mode": "github",
                "github_repo": "acme/shared-shellshelf",
                "teams_dir": "company-teams",
                "default_team": "platform"
            },
            "web": {
                "port": 4920,
                "theme": "giphy"
            }
        })
    );
}

#[test]
fn test_add_repo_rejects_combined_flags() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path())
        .args(["--add-repo", "acme/shared-shellshelf", "--list"]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "--add-repo must be used on its own.",
    ));
}

#[test]
fn test_open_pr_requires_team() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path()).args([
        "-s",
        "curl",
        "--add",
        "curl https://example.com",
        "--open-pr",
    ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "--open-pr requires --team so the change targets shared storage.",
    ));
}

#[test]
fn test_pr_branch_requires_open_pr() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("shellshelf").unwrap();

    cmd.env("HOME", temp_dir.path()).args([
        "-s",
        "curl",
        "--add",
        "curl https://example.com",
        "--pr-branch",
        "feat/curl",
    ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "--pr-branch can only be used with --open-pr.",
    ));
}

#[test]
fn test_default_list_shelves_includes_all_teams_when_shared_repo_is_configured() {
    let temp_dir = TempDir::new().unwrap();
    let home_dir = temp_dir.path();
    let shellshelf_dir = home_dir.join(".shellshelf");
    let config_path = shellshelf_dir.join("config.json");
    let shared_repo = home_dir.join("shared-shellshelf");

    fs::create_dir_all(&shellshelf_dir).unwrap();
    write_path_config(&config_path, &shared_repo, "teams", None, None, None, None);
    write_command_database(
        &shared_repo
            .join("teams")
            .join("platform")
            .join("shelves")
            .join("curl.json"),
        &[("curl https://platform.example.com/health", Some("Platform"))],
    );
    write_command_database(
        &shared_repo
            .join("teams")
            .join("infra")
            .join("shelves")
            .join("media.json"),
        &[("curl https://infra.example.com/media", Some("Infra"))],
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", home_dir).arg("--list-shelves");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("=== SHARED / INFRA ==="))
        .stdout(predicate::str::contains("=== SHARED / PLATFORM ==="))
        .stdout(predicate::str::contains("[1] media"))
        .stdout(predicate::str::contains("[1] curl"));
}

#[test]
fn test_import_postman_creates_local_shelf() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "postman-api",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "List workspaces",
      "request": {
        "method": "GET",
        "url": "https://api.getpostman.com/workspaces"
      }
    }
  ]
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["--import-postman", import_file.to_str().unwrap()]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Imported 1 request into shelf 'postman-api'. Skipped 0 requests.",
    ));

    let data_file = temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("postman-api.json");
    assert!(data_file.exists());

    let content = fs::read_to_string(data_file).unwrap();
    assert!(content.contains("https://api.getpostman.com/workspaces"));
    assert!(content.contains("List workspaces"));
}

#[test]
fn test_import_postman_creates_team_shelf() {
    let temp_dir = TempDir::new().unwrap();
    let shared_repo = temp_dir.path().join("shared-shellshelf");
    let import_file = temp_dir.path().join("team-postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "platform-api",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "Health",
      "request": {
        "method": "GET",
        "url": "https://api.example.com/platform/health"
      }
    }
  ]
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--repo",
        shared_repo.to_str().unwrap(),
        "--team",
        "platform",
        "--import-postman",
        import_file.to_str().unwrap(),
    ]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Imported 1 request into shelf 'platform-api' for team 'platform'. Skipped 0 requests.",
    ));

    assert!(shared_repo
        .join("teams")
        .join("platform")
        .join("shelves")
        .join("platform-api.json")
        .exists());
}

#[test]
fn test_import_postman_shelf_override() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("override-postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "ignored-name",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "Ping",
      "request": {
        "method": "GET",
        "url": "https://example.com/ping"
      }
    }
  ]
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--target-shelf",
        "curl",
        "--import-postman",
        import_file.to_str().unwrap(),
    ]);

    cmd.assert().success().stdout(predicate::str::contains(
        "Imported 1 request into shelf 'curl'. Skipped 0 requests.",
    ));

    assert!(temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("curl.json")
        .exists());
}

#[test]
fn test_import_postman_rejects_existing_shelf() {
    let temp_dir = TempDir::new().unwrap();
    let existing_shelf = temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("curl.json");
    write_command_database(
        &existing_shelf,
        &[("curl https://existing.example.com", None)],
    );

    let import_file = temp_dir.path().join("existing-postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "curl",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "New request",
      "request": {
        "method": "GET",
        "url": "https://example.com/new"
      }
    }
  ]
}"#,
    );

    let original_content = fs::read_to_string(&existing_shelf).unwrap();

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["--import-postman", import_file.to_str().unwrap()]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains(
            "Shelf 'curl' already exists. Use --target-shelf <NAME> to choose a different shelf name for this import.",
        ));

    assert_eq!(
        fs::read_to_string(existing_shelf).unwrap(),
        original_content
    );
}

#[test]
fn test_import_postman_rejects_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("invalid-postman.json");
    write_text_file(&import_file, "{invalid");

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["--import-postman", import_file.to_str().unwrap()]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "Failed to parse Postman collection JSON",
    ));

    assert!(!temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("invalid-postman.json")
        .exists());
}

#[test]
fn test_import_postman_rejects_unsupported_schema() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("unsupported-schema.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "curl",
    "schema": "https://schema.getpostman.com/json/collection/v2.0.0/collection.json"
  },
  "item": []
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["--import-postman", import_file.to_str().unwrap()]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "Unsupported Postman collection schema",
    ));

    assert!(!temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("curl.json")
        .exists());
}

#[test]
fn test_import_postman_warns_on_partial_success() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("partial-postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "mixed-api",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "Supported",
      "request": {
        "method": "GET",
        "url": "https://example.com/supported"
      }
    },
    {
      "name": "Repair GIF",
      "request": {
        "method": "POST",
        "body": {
          "mode": "formdata",
          "formdata": [
            { "key": "media_id", "type": "text", "value": "abc123" },
            { "key": "file", "type": "file", "src": ["/tmp/one.gif", "/tmp/two.gif"] }
          ]
        },
        "url": "https://example.com/repair"
      }
    }
  ]
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["--import-postman", import_file.to_str().unwrap()]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Imported 1 request into shelf 'mixed-api'. Skipped 1 request.",
        ))
        .stderr(predicate::str::contains(
            "Warning: skipped 1 request during Postman import.",
        ))
        .stderr(predicate::str::contains(
            "Repair GIF: contains unsupported form-data parts; skipping whole request: form-data file field 'file' is missing a supported file path",
        ));
}

#[test]
fn test_import_postman_supports_formdata_requests() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("media-postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "media",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "Repair GIF",
      "request": {
        "method": "POST",
        "body": {
          "mode": "formdata",
          "formdata": [
            { "key": "media_id", "type": "text", "value": "abc123" },
            { "key": "notes", "type": "text", "value": "" },
            { "key": "file", "type": "file", "src": "/tmp/test.gif" }
          ]
        },
        "url": "https://example.com/repair"
      }
    }
  ]
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["--import-postman", import_file.to_str().unwrap()]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Imported 1 request into shelf 'media'. Skipped 0 requests.",
        ))
        .stderr(predicate::str::is_empty());

    let content = fs::read_to_string(
        temp_dir
            .path()
            .join(".shellshelf")
            .join("shelves")
            .join("media.json"),
    )
    .unwrap();
    assert!(content.contains("-F 'media_id=abc123'"));
    assert!(content.contains("-F 'notes='"));
    assert!(content.contains("-F 'file=@/tmp/test.gif'"));
}

#[test]
fn test_import_postman_fails_when_every_request_is_unsupported() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("unsupported-postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "secure-api",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "Secured request",
      "request": {
        "method": "GET",
        "auth": {
          "type": "bearer"
        },
        "url": "https://example.com/secure"
      }
    }
  ]
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path())
        .args(["--import-postman", import_file.to_str().unwrap()]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains(
            "Postman import failed: no supported requests were found.",
        ))
        .stderr(predicate::str::contains(
            "Secured request: uses auth or auth inheritance, which is not supported yet",
        ));

    assert!(!temp_dir
        .path()
        .join(".shellshelf")
        .join("shelves")
        .join("secure-api.json")
        .exists());
}

#[test]
fn test_import_postman_rejects_invalid_flag_combinations() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("flags-postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "curl",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": []
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--import-postman",
        import_file.to_str().unwrap(),
        "--list",
    ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "--list cannot be combined with --import-postman.",
    ));
}

#[test]
fn test_import_postman_rejects_shelf_flag_and_points_to_target_shelf() {
    let temp_dir = TempDir::new().unwrap();
    let import_file = temp_dir.path().join("shelf-flag-postman.json");
    write_text_file(
        &import_file,
        r#"{
  "info": {
    "name": "curl",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": []
}"#,
    );

    let mut cmd = Command::cargo_bin("shellshelf").unwrap();
    cmd.env("HOME", temp_dir.path()).args([
        "--shelf",
        "curl",
        "--import-postman",
        import_file.to_str().unwrap(),
    ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "--shelf cannot be used with --import-postman. Use --target-shelf instead.",
    ));
}
