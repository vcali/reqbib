use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

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
        .stdout(predicate::str::contains("--list"));
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
        .stdout(predicate::str::contains("All stored curl commands (1):"))
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
        .stdout(predicate::str::contains(
            "Found 2 matching curl command(s):",
        ))
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

    cmd.assert().success().stdout(predicate::str::contains(
        "No curl commands found matching keywords: nonexistent",
    ));
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
        .stdout(predicate::str::contains(
            "Found 1 matching curl command(s):",
        ))
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
        .stdout(predicate::str::contains("All stored curl commands (3):"))
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
        .stdout(predicate::str::contains("All stored curl commands (1):"));
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
        .stdout(predicate::str::contains("All stored curl commands (1):"));
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
        .stdout(predicate::str::contains(
            "Found 2 matching curl command(s):",
        ))
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
        .stdout(predicate::str::contains(
            "Found 2 matching curl command(s):",
        ))
        .stdout(predicate::str::contains("api.github.com/users"))
        .stdout(predicate::str::contains("api.github.com/repos"));

    // Test filtering with very specific keywords
    let mut cmd = Command::cargo_bin("reqbib").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.args(["-l", "github", "POST"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Found 1 matching curl command(s):",
        ))
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

    cmd.assert().success().stdout(predicate::str::contains(
        "No curl commands found matching keywords: nonexistent",
    ));
}
