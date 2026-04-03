use crate::Result;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Output};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedPublishBranch {
    pub(crate) base_branch: String,
    pub(crate) pr_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublishPullRequestPlan {
    pub(crate) commit_message: String,
    pub(crate) pr_title: String,
    pub(crate) pr_body: String,
    pub(crate) changed_paths: Vec<PathBuf>,
}

pub(crate) fn prepare_publish_branch(
    repo_root: &Path,
    requested_base_branch: Option<&str>,
    requested_pr_branch: Option<&str>,
    default_pr_branch: &str,
) -> Result<PreparedPublishBranch> {
    ensure_clean_worktree(repo_root)?;

    let base_branch = match requested_base_branch {
        Some(branch) => validate_branch_input(branch, "--base-branch")?.to_string(),
        None => detect_default_base_branch(repo_root)?,
    };

    let current_branch = current_branch(repo_root)?;
    let pr_branch = match requested_pr_branch {
        Some(branch) => validate_branch_input(branch, "--pr-branch")?.to_string(),
        None if !current_branch.is_empty() && current_branch != base_branch => {
            current_branch.clone()
        }
        None => validate_branch_input(default_pr_branch, "generated publish branch")?.to_string(),
    };

    if pr_branch == base_branch {
        return Err("The publish branch must differ from the base branch.".into());
    }

    run_git(repo_root, ["fetch", "origin", base_branch.as_str()])?;

    if current_branch != pr_branch {
        if local_branch_exists(repo_root, &pr_branch)? {
            run_git(repo_root, ["switch", pr_branch.as_str()])?;
        } else {
            run_git(repo_root, ["switch", "-c", pr_branch.as_str()])?;
        }
    }

    let upstream_base = format!("origin/{base_branch}");
    run_git(repo_root, ["rebase", upstream_base.as_str()])?;

    Ok(PreparedPublishBranch {
        base_branch,
        pr_branch,
    })
}

pub(crate) fn publish_pull_request(
    repo_root: &Path,
    prepared_branch: &PreparedPublishBranch,
    plan: &PublishPullRequestPlan,
) -> Result<Option<String>> {
    if !paths_have_changes(repo_root, &plan.changed_paths)? {
        return Ok(None);
    }

    run_git_with_os_args(
        repo_root,
        std::iter::once(OsString::from("add")).chain(
            plan.changed_paths
                .iter()
                .map(|path| path.as_os_str().to_os_string()),
        ),
    )?;

    run_git(repo_root, ["commit", "-m", plan.commit_message.as_str()])?;
    run_git(
        repo_root,
        [
            "push",
            "--set-upstream",
            "origin",
            prepared_branch.pr_branch.as_str(),
        ],
    )?;

    let output = run_gh(
        repo_root,
        [
            "pr",
            "create",
            "--base",
            prepared_branch.base_branch.as_str(),
            "--head",
            prepared_branch.pr_branch.as_str(),
            "--title",
            plan.pr_title.as_str(),
            "--body",
            plan.pr_body.as_str(),
        ],
    )?;

    let url = output.trim();
    if url.is_empty() {
        Ok(Some(format!(
            "{} -> {}",
            prepared_branch.pr_branch, prepared_branch.base_branch
        )))
    } else {
        Ok(Some(url.to_string()))
    }
}

pub(crate) fn sanitize_branch_component(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if matches!(ch, '-' | '_' | '.') {
            slug.push(ch);
            previous_was_separator = false;
        } else if !previous_was_separator {
            slug.push('-');
            previous_was_separator = true;
        }
    }

    let trimmed = slug
        .trim_matches(|ch| matches!(ch, '-' | '_' | '.'))
        .to_string();

    if trimmed.is_empty() {
        "update".to_string()
    } else {
        trimmed
    }
}

fn validate_branch_input<'a>(value: &'a str, source: &str) -> Result<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{source} cannot be empty.").into());
    }
    Ok(trimmed)
}

fn detect_default_base_branch(repo_root: &Path) -> Result<String> {
    if let Ok(symbolic_ref) = run_git(repo_root, ["symbolic-ref", "refs/remotes/origin/HEAD"]) {
        let trimmed = symbolic_ref.trim();
        if let Some(branch) = trimmed.strip_prefix("refs/remotes/origin/") {
            return Ok(branch.to_string());
        }
    }

    for branch in ["main", "master"] {
        let remote_ref = format!("refs/remotes/origin/{branch}");
        if git_exit_status(
            repo_root,
            ["show-ref", "--verify", "--quiet", remote_ref.as_str()],
        )?
        .success()
        {
            return Ok(branch.to_string());
        }
    }

    Err("Could not determine the shared repository base branch. Use --base-branch <BRANCH>.".into())
}

fn ensure_clean_worktree(repo_root: &Path) -> Result<()> {
    let status = run_git(repo_root, ["status", "--porcelain"])?;
    if status.trim().is_empty() {
        Ok(())
    } else {
        Err(
            "Shared repository checkout has uncommitted changes. Commit, stash, or discard them before using --open-pr."
                .into(),
        )
    }
}

fn current_branch(repo_root: &Path) -> Result<String> {
    Ok(run_git(repo_root, ["branch", "--show-current"])?
        .trim()
        .to_string())
}

fn local_branch_exists(repo_root: &Path, branch: &str) -> Result<bool> {
    let branch_ref = format!("refs/heads/{branch}");
    Ok(git_exit_status(
        repo_root,
        ["show-ref", "--verify", "--quiet", branch_ref.as_str()],
    )?
    .success())
}

fn paths_have_changes(repo_root: &Path, changed_paths: &[PathBuf]) -> Result<bool> {
    let status = run_git_with_os_args(
        repo_root,
        std::iter::once(OsString::from("status"))
            .chain(std::iter::once(OsString::from("--porcelain")))
            .chain(std::iter::once(OsString::from("--")))
            .chain(
                changed_paths
                    .iter()
                    .map(|path| path.as_os_str().to_os_string()),
            ),
    )?;

    Ok(!status.trim().is_empty())
}

fn run_git(repo_root: &Path, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Result<String> {
    run_command(
        repo_root,
        &git_binary(),
        args,
        "Shared repository publishing requires Git to be installed.",
    )
}

fn run_git_with_os_args(
    repo_root: &Path,
    args: impl IntoIterator<Item = OsString>,
) -> Result<String> {
    run_command(
        repo_root,
        &git_binary(),
        args,
        "Shared repository publishing requires Git to be installed.",
    )
}

fn git_exit_status(
    repo_root: &Path,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<std::process::ExitStatus> {
    Ok(run_command_output(
        repo_root,
        &git_binary(),
        args,
        "Shared repository publishing requires Git to be installed.",
    )?
    .status)
}

fn run_gh(repo_root: &Path, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Result<String> {
    run_command(
        repo_root,
        &gh_binary(),
        args,
        "Shared repository publishing requires the GitHub CLI to be installed and authenticated.",
    )
}

fn run_command(
    repo_root: &Path,
    binary: &str,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    install_message: &str,
) -> Result<String> {
    let output = run_command_output(repo_root, binary, args, install_message)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} failed: {}", binary, stderr.trim()).into())
    }
}

fn run_command_output(
    repo_root: &Path,
    binary: &str,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    install_message: &str,
) -> Result<Output> {
    let args: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    ProcessCommand::new(binary)
        .current_dir(repo_root)
        .args(&args)
        .output()
        .map_err(|error| format!("Failed to execute '{binary}'. {install_message} {error}").into())
}

fn git_binary() -> String {
    env::var("SHELLSHELF_GIT_BIN").unwrap_or_else(|_| "git".to_string())
}

fn gh_binary() -> String {
    env::var("SHELLSHELF_GH_BIN").unwrap_or_else(|_| "gh".to_string())
}

#[cfg(test)]
mod tests {
    use super::sanitize_branch_component;

    #[test]
    fn test_sanitize_branch_component_normalizes_text() {
        assert_eq!(
            sanitize_branch_component("Platform Uploads"),
            "platform-uploads"
        );
        assert_eq!(sanitize_branch_component("curl.aws"), "curl.aws");
    }

    #[test]
    fn test_sanitize_branch_component_falls_back_when_empty() {
        assert_eq!(sanitize_branch_component("!!!"), "update");
    }
}
