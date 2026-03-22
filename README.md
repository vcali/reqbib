# reqbib

ReqBib is a CLI for storing, searching, and sharing `curl` commands.

The name comes from **Requests Biblioteca**: a library of useful HTTP requests that you can keep for yourself or organize with a team.

## What It Does

- Store `curl` commands locally
- Use a shared GitHub repository for team-scoped command libraries
- Search across all teams in a shared repository
- Import `curl` commands from your shell history
- Search commands by extracted keywords
- Prevent duplicate entries
- Store team-specific command libraries inside a shared repository layout

## GitHub-Backed Team Storage

ReqBib can operate as a local personal tool, but it now also supports a GitHub-backed team workflow:

- configure a shared repository in `~/.reqbib/config.json`
- let ReqBib bootstrap a local checkout with `gh repo clone`
- store team commands under `teams/<team>/commands.json`
- keep managed checkouts refreshed automatically unless disabled in config

Quick example:

```json
{
  "github_repo": "acme/shared-reqbib",
  "teams_dir": "teams",
  "auto_update_repo": true
}
```

```bash
reqbib --team platform -a "curl https://api.example.com/platform/health"
reqbib --team platform -l
```


## Current Storage Modes

### Local mode

By default, ReqBib stores commands in:

```text
~/.reqbib/commands.json
```

### Shared repository mode

ReqBib can also write to a shared repository checkout using a team-based layout:

```text
shared-reqbib/
  teams/
    platform/
      commands.json
    payments/
      commands.json
```

Use this mode with `--repo <path>` and `--team <team-name>`.

This is the current GitHub integration model: the shared repository can live on GitHub, teammates collaborate through the repository structure, ReqBib can bootstrap a local checkout via `gh repo clone`, and managed checkouts can be refreshed automatically. ReqBib still does not manage authentication, commit, or push workflows by itself.

## GitHub Integration Prerequisite

For GitHub-backed team storage, ReqBib relies on the GitHub CLI:

```bash
gh auth status
```

That means:

- `gh` must be installed
- `gh` must already be authenticated for the repositories you want to use
- `git` must be available for periodic local checkout updates

Local-only usage does not require `gh`.

If GitHub-backed team storage is the main use case, this is the minimum setup:

1. Install and authenticate `gh`
2. Ensure `git` is available
3. Configure `github_repo` in `~/.reqbib/config.json`
4. Use `reqbib --team <team> ...`

## Configuration

ReqBib can read local defaults from:

```text
~/.reqbib/config.json
```

Example:

```json
{
  "github_repo": "acme/shared-reqbib",
  "shared_repo_path": "/Users/alice/src/shared-reqbib",
  "teams_dir": "teams",
  "auto_update_repo": true
}
```

Supported keys:

- `github_repo`: GitHub repository identifier such as `acme/shared-reqbib`; if `shared_repo_path` is not set, ReqBib will use `gh repo clone` to create a local checkout under `~/.reqbib/repos/`
- `shared_repo_path`: Local checkout path for the shared repository
- `teams_dir`: Relative path to the teams directory inside that repository
- `auto_update_repo`: When `true` or omitted, ReqBib will periodically run `git pull --ff-only` on managed GitHub checkouts; set to `false` to disable that behavior

Override order:

1. CLI arguments
2. Local config file
3. Built-in defaults

Today, the config is mainly used for shared repository mode. The local command database still defaults to `~/.reqbib/commands.json`.

Managed GitHub checkouts are refreshed automatically on a fixed interval. Today that interval is 15 minutes.

## Usage

### Show help

```bash
reqbib
```

### Add a command locally

```bash
reqbib -a "curl -I https://api.github.com/users/octocat"
```

### Search locally

```bash
reqbib github octocat
```

### List all local commands

```bash
reqbib -l
```

### Import from shell history

```bash
reqbib -i
```

### Add a command to a team repository

```bash
reqbib --repo /path/to/shared-reqbib --team platform \
  -a "curl https://api.example.com/platform/health"
```

### Use configured repository defaults

If `shared_repo_path` is set in `~/.reqbib/config.json`, you can just pass the team:

```bash
reqbib --team platform -l
```

### Use GitHub repo config with automatic checkout

If `github_repo` is set and `shared_repo_path` is not, ReqBib will use `gh repo clone` on first use:

```bash
reqbib --team platform -l
```

### Override only the teams directory

```bash
reqbib --team platform --teams-dir company-teams -l
```

### List commands for one team

```bash
reqbib --repo /path/to/shared-reqbib --team platform -l
```

### Search within one team

```bash
reqbib --repo /path/to/shared-reqbib --team payments stripe webhook
```

### Search across all teams

```bash
reqbib --repo /path/to/shared-reqbib --all-teams stripe webhook
```

### List commands across all teams

```bash
reqbib --repo /path/to/shared-reqbib --all-teams -l
```

## How Search Works

ReqBib extracts useful keywords from each command, including:

- Domain names and subdomains
- URL path segments
- HTTP methods
- Header names and values
- Other meaningful words in the command

Search is case-insensitive and supports multiple keywords.

## Sensitive Data Warning

Some `curl` commands contain secrets such as:

- `Authorization` headers
- Bearer tokens
- Cookies
- API keys
- Session identifiers

Today, ReqBib stores commands as provided. If you use shared repository mode, be careful not to commit live credentials. Secret detection and redaction are planned next and should be treated as a high-priority safety feature.

## Building

```bash
cargo build
```

Release build:

```bash
cargo build --release
```

## Running During Development

```bash
cargo run
```

Pass arguments after `--`:

```bash
cargo run -- -l
```

Examples:

```bash
cargo run -- -a "curl https://example.com"
cargo run -- github api
cargo run -- --team platform -l
cargo run -- --repo /path/to/shared-reqbib --team platform -l
```

## Installing Locally

```bash
cargo install --path .
```

## Development Checks

Run the same checks used by CI:

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --document-private-items
```
