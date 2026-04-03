use crate::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{Duration, SystemTime};

pub(crate) const DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES: u64 = 15;

pub(crate) fn normalize_github_repo_input(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("GitHub repository cannot be empty.".into());
    }

    let stripped = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("https://www.github.com/"))
        .or_else(|| trimmed.strip_prefix("http://www.github.com/"))
        .or_else(|| trimmed.strip_prefix("github.com/"))
        .or_else(|| trimmed.strip_prefix("git@github.com:"))
        .unwrap_or(trimmed);

    let normalized = stripped
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_string();

    validate_github_repo_name(&normalized)?;
    Ok(normalized)
}

pub(crate) fn validate_github_repo_name(repo: &str) -> Result<()> {
    let Some((owner, name)) = repo.split_once('/') else {
        return Err("GitHub repository must be in the format <owner>/<repo>.".into());
    };

    let is_valid_part = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    };

    if is_valid_part(owner) && is_valid_part(name) {
        Ok(())
    } else {
        Err("GitHub repository must be in the format <owner>/<repo> using only letters, numbers, dots, underscores, and hyphens.".into())
    }
}

pub(crate) fn get_default_github_checkout_root() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".shellshelf");
    path.push("repos");
    path
}

pub(crate) fn get_default_github_state_root() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".shellshelf");
    path.push("state");
    path
}

fn github_repo_slug(github_repo: &str) -> Result<String> {
    validate_github_repo_name(github_repo)?;
    let (owner, repo) = github_repo
        .split_once('/')
        .ok_or("GitHub repository must be in the format <owner>/<repo>.")?;
    Ok(format!("{owner}__{repo}"))
}

pub(crate) fn get_github_repo_checkout_path(
    repository_root: &Path,
    github_repo: &str,
) -> Result<PathBuf> {
    Ok(repository_root.join(github_repo_slug(github_repo)?))
}

pub(crate) fn get_github_repo_sync_stamp_path(
    state_root: &Path,
    github_repo: &str,
) -> Result<PathBuf> {
    Ok(state_root.join(format!("{}.sync", github_repo_slug(github_repo)?)))
}

fn clone_github_repo(github_repo: &str, checkout_path: &Path) -> Result<()> {
    let gh_binary = env::var("SHELLSHELF_GH_BIN").unwrap_or_else(|_| "gh".to_string());
    let output = ProcessCommand::new(&gh_binary)
        .arg("repo")
        .arg("clone")
        .arg(github_repo)
        .arg(checkout_path)
        .output()
        .map_err(|error| {
            format!(
                "Failed to execute '{gh_binary}'. GitHub integration requires the GitHub CLI to be installed and authenticated: {error}"
            )
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("gh repo clone failed: {}", stderr.trim()).into())
    }
}

fn pull_github_repo(checkout_path: &Path) -> Result<()> {
    let git_binary = env::var("SHELLSHELF_GIT_BIN").unwrap_or_else(|_| "git".to_string());
    let output = ProcessCommand::new(&git_binary)
        .arg("-C")
        .arg(checkout_path)
        .arg("pull")
        .arg("--ff-only")
        .output()
        .map_err(|error| {
            format!(
                "Failed to execute '{git_binary}'. Automatic repository updates require Git to be installed: {error}"
            )
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("git pull --ff-only failed: {}", stderr.trim()).into())
    }
}

fn should_auto_update_repo(sync_stamp_path: &Path, auto_update_interval: Duration) -> bool {
    let modified_time = fs::metadata(sync_stamp_path)
        .and_then(|metadata| metadata.modified())
        .ok();

    match modified_time {
        Some(modified_time) => match SystemTime::now().duration_since(modified_time) {
            Ok(elapsed) => elapsed >= auto_update_interval,
            Err(_) => true,
        },
        None => true,
    }
}

pub(crate) fn write_github_repo_sync_stamp(state_root: &Path, github_repo: &str) -> Result<()> {
    let sync_stamp_path = get_github_repo_sync_stamp_path(state_root, github_repo)?;
    fs::create_dir_all(state_root)?;
    fs::write(sync_stamp_path, b"updated")?;
    Ok(())
}

pub(crate) fn maybe_update_github_repo_checkout_with_runner<F>(
    github_repo: &str,
    checkout_path: &Path,
    auto_update_repo: bool,
    auto_update_interval: Duration,
    state_root: &Path,
    update_runner: F,
) -> Result<bool>
where
    F: FnOnce(&Path) -> Result<()>,
{
    if !auto_update_repo {
        return Ok(false);
    }

    let sync_stamp_path = get_github_repo_sync_stamp_path(state_root, github_repo)?;
    if !should_auto_update_repo(&sync_stamp_path, auto_update_interval) {
        return Ok(false);
    }

    update_runner(checkout_path)?;
    write_github_repo_sync_stamp(state_root, github_repo)?;
    Ok(true)
}

pub(crate) fn maybe_update_github_repo_checkout(
    github_repo: &str,
    checkout_path: &Path,
    auto_update_repo: bool,
    auto_update_interval: Duration,
) -> Result<bool> {
    let state_root = get_default_github_state_root();
    maybe_update_github_repo_checkout_with_runner(
        github_repo,
        checkout_path,
        auto_update_repo,
        auto_update_interval,
        &state_root,
        pull_github_repo,
    )
}

pub(crate) fn ensure_github_repo_checkout_with_runner<F>(
    github_repo: &str,
    checkout_root: &Path,
    clone_runner: F,
) -> Result<(PathBuf, bool)>
where
    F: FnOnce(&str, &Path) -> Result<()>,
{
    let checkout_path = get_github_repo_checkout_path(checkout_root, github_repo)?;

    if checkout_path.exists() {
        return Ok((checkout_path, false));
    }

    fs::create_dir_all(checkout_root)?;
    clone_runner(github_repo, &checkout_path)?;
    Ok((checkout_path, true))
}

pub(crate) fn ensure_github_repo_checkout(github_repo: &str) -> Result<(PathBuf, bool)> {
    let checkout_root = get_default_github_checkout_root();
    ensure_github_repo_checkout_with_runner(github_repo, &checkout_root, clone_github_repo)
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_github_repo_checkout_with_runner, get_github_repo_checkout_path,
        get_github_repo_sync_stamp_path, maybe_update_github_repo_checkout_with_runner,
        normalize_github_repo_input, validate_github_repo_name,
        DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES,
    };
    use std::fs;
    use std::path::Path;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_validate_github_repo_name_accepts_owner_repo() {
        validate_github_repo_name("acme/shared-shellshelf").unwrap();
    }

    #[test]
    fn test_normalize_github_repo_input_accepts_common_url_forms() {
        assert_eq!(
            normalize_github_repo_input("https://github.com/acme/shared-shellshelf.git").unwrap(),
            "acme/shared-shellshelf"
        );
        assert_eq!(
            normalize_github_repo_input("git@github.com:acme/shared-shellshelf.git").unwrap(),
            "acme/shared-shellshelf"
        );
        assert_eq!(
            normalize_github_repo_input("github.com/acme/shared-shellshelf/").unwrap(),
            "acme/shared-shellshelf"
        );
    }

    #[test]
    fn test_normalize_github_repo_input_rejects_empty_input() {
        let error = normalize_github_repo_input("   ").expect_err("empty repo should fail");
        assert_eq!(error.to_string(), "GitHub repository cannot be empty.");
    }

    #[test]
    fn test_validate_github_repo_name_rejects_invalid_format() {
        let error =
            validate_github_repo_name("acme").expect_err("missing repo name should be rejected");

        assert_eq!(
            error.to_string(),
            "GitHub repository must be in the format <owner>/<repo>."
        );
    }

    #[test]
    fn test_get_github_repo_checkout_path() {
        let checkout_path = get_github_repo_checkout_path(
            Path::new("/tmp/shellshelf-repos"),
            "acme/shared-shellshelf",
        )
        .unwrap();

        assert_eq!(
            checkout_path,
            Path::new("/tmp/shellshelf-repos").join("acme__shared-shellshelf")
        );
    }

    #[test]
    fn test_ensure_github_repo_checkout_with_runner_clones_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_root = temp_dir.path().join("repos");

        let (checkout_path, was_cloned) = ensure_github_repo_checkout_with_runner(
            "acme/shared-shellshelf",
            &checkout_root,
            |repo, destination| {
                assert_eq!(repo, "acme/shared-shellshelf");
                fs::create_dir_all(destination)?;
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(checkout_path, checkout_root.join("acme__shared-shellshelf"));
        assert!(checkout_path.exists());
        assert!(was_cloned);
    }

    #[test]
    fn test_ensure_github_repo_checkout_with_runner_uses_existing_checkout() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_root = temp_dir.path().join("repos");
        let existing_checkout = checkout_root.join("acme__shared-shellshelf");
        fs::create_dir_all(&existing_checkout).unwrap();

        let (checkout_path, was_cloned) = ensure_github_repo_checkout_with_runner(
            "acme/shared-shellshelf",
            &checkout_root,
            |_repo, _destination| Err("clone should not be called".into()),
        )
        .unwrap();

        assert_eq!(checkout_path, existing_checkout);
        assert!(!was_cloned);
    }

    #[test]
    fn test_get_github_repo_sync_stamp_path() {
        let sync_stamp_path = get_github_repo_sync_stamp_path(
            Path::new("/tmp/shellshelf-state"),
            "acme/shared-shellshelf",
        )
        .unwrap();

        assert_eq!(
            sync_stamp_path,
            Path::new("/tmp/shellshelf-state").join("acme__shared-shellshelf.sync")
        );
    }

    #[test]
    fn test_maybe_update_github_repo_checkout_with_runner_updates_when_due() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_path = temp_dir.path().join("acme__shared-shellshelf");
        let state_root = temp_dir.path().join("state");
        fs::create_dir_all(&checkout_path).unwrap();

        let was_updated = maybe_update_github_repo_checkout_with_runner(
            "acme/shared-shellshelf",
            &checkout_path,
            true,
            Duration::from_secs(DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES * 60),
            &state_root,
            |path| {
                assert_eq!(path, checkout_path.as_path());
                Ok(())
            },
        )
        .unwrap();

        assert!(was_updated);
        assert!(state_root.join("acme__shared-shellshelf.sync").exists());
    }

    #[test]
    fn test_maybe_update_github_repo_checkout_with_runner_respects_disable_flag() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_path = temp_dir.path().join("acme__shared-shellshelf");
        let state_root = temp_dir.path().join("state");
        fs::create_dir_all(&checkout_path).unwrap();

        let was_updated = maybe_update_github_repo_checkout_with_runner(
            "acme/shared-shellshelf",
            &checkout_path,
            false,
            Duration::from_secs(DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES * 60),
            &state_root,
            |_path| Err("update should not run".into()),
        )
        .unwrap();

        assert!(!was_updated);
        assert!(!state_root.exists());
    }

    #[test]
    fn test_maybe_update_github_repo_checkout_with_runner_skips_recent_sync() {
        let temp_dir = TempDir::new().unwrap();
        let checkout_path = temp_dir.path().join("acme__shared-shellshelf");
        let state_root = temp_dir.path().join("state");
        let sync_stamp_path = state_root.join("acme__shared-shellshelf.sync");
        fs::create_dir_all(&checkout_path).unwrap();
        fs::create_dir_all(&state_root).unwrap();
        fs::write(&sync_stamp_path, b"updated").unwrap();

        let was_updated = maybe_update_github_repo_checkout_with_runner(
            "acme/shared-shellshelf",
            &checkout_path,
            true,
            Duration::from_secs(DEFAULT_GITHUB_REPO_AUTO_UPDATE_INTERVAL_MINUTES * 60),
            &state_root,
            |_path| Err("update should not run".into()),
        )
        .unwrap();

        assert!(!was_updated);
    }
}
