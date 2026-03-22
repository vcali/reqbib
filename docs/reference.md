# ReqBib Reference

This document covers configuration, CLI parameters, storage modes, and the current GitHub-backed shared repository workflow.

## Storage Modes

### Local mode

Default storage path:

```text
~/.reqbib/commands.json
```

Use local mode when you do not pass `--team` or `--all-teams`.

If `shared_repo.default_team` is configured, default `reqbib -l` and `reqbib <keywords...>` reads include local commands plus that team. If `shared_repo.default_all_teams` is `true`, default reads include local commands plus every team.

### Shared repository mode

Shared mode stores commands under a repository layout like this:

```text
shared-reqbib/
  teams/
    platform/
      commands.json
    payments/
      commands.json
```

Shared mode is activated when:

- `--team <team>` is used
- `--all-teams` is used

Repository resolution order:

1. `--repo <path>`
2. `shared_repo.mode = "path"` from config
3. `shared_repo.mode = "github"` from config, which bootstraps a managed local checkout with `gh repo clone`

## Configuration

Default config location:

```text
~/.reqbib/config.json
```

You can override that with:

```bash
reqbib --config /path/to/config.json ...
```

### Path mode

```json
{
  "shared_repo": {
    "mode": "path",
    "path": "/Users/alice/src/shared-reqbib",
    "teams_dir": "teams"
  }
}
```

### GitHub mode

```json
{
  "shared_repo": {
    "mode": "github",
    "github_repo": "acme/shared-reqbib",
    "teams_dir": "teams",
    "default_team": "platform",
    "auto_update_repo": true,
    "auto_update_interval_minutes": 15
  },
  "default_list_limit": 20
}
```

Supported keys inside `shared_repo`:

- `mode`: must be `path` or `github`
- `path`: required for `mode = "path"`
- `github_repo`: required for `mode = "github"`, in `<owner>/<repo>` format
- `teams_dir`: relative path inside the repository that contains team folders. Defaults to `teams`
- `default_team`: optional default shared team for non-team read commands
- `default_all_teams`: optional shared read default for non-team read commands. Defaults to `false`
- `auto_update_repo`: GitHub mode only. Defaults to `true`
- `auto_update_interval_minutes`: GitHub mode only. Defaults to `15` and must be greater than `0`

Supported top-level keys:

- `shared_repo`: shared repository configuration
- `default_list_limit`: optional default limit for `--list`. `0` means unlimited. If omitted, ReqBib defaults to `20`

Validation rules:

- `mode = "path"` requires `path` and rejects `github_repo`, `auto_update_repo`, and `auto_update_interval_minutes`
- `mode = "github"` requires `github_repo` and rejects `path`
- `default_team` and `default_all_teams = true` cannot be configured together
- flat legacy config keys such as `github_repo`, `shared_repo_path`, `teams_dir`, and `auto_update_repo` at the top level are rejected

Precedence:

1. CLI arguments
2. Config file
3. Built-in defaults

## CLI Parameters

### Core operations

- `-a`, `--add <CURL_COMMAND>`: add a command to the active storage target
- `--description <TEXT>`: optional brief description for `--add`
- `-i`, `--import`: import `curl` commands from shell history
- `-l`, `--list`: list commands in the active storage target
- `<keywords...>`: search for commands by keyword

### Shared repository options

- `--repo <PATH>`: path to a shared repository checkout
- `--team <TEAM>`: target a single team folder
- `--teams-dir <PATH>`: relative path to the teams directory within the repo
- `--all-teams`: list or search across every team in the repository
- `--local-only`: limit default list/search commands to local storage
- `--shared-only`: limit default list/search commands to shared storage
- `--limit <COUNT>`: limit how many commands are shown with `--list`. `0` means unlimited

### Configuration option

- `--config <PATH>`: use a non-default ReqBib config file

## Shared Mode Rules

- `--team` and `--all-teams` cannot be used together
- `--local-only` and `--shared-only` cannot be used together
- `--all-teams` is read-only
- `--all-teams` cannot be used with `--add`
- `--all-teams` cannot be used with `--import`
- `--repo` and `--teams-dir` may be used for default read commands without `--team`
- `--repo` and `--teams-dir` still require `--team` for write commands
- `--description` can only be used with `--add`
- `--local-only` and `--shared-only` are read-only controls and cannot be used with `--add` or `--import`
- `--local-only` and `--shared-only` cannot be used with `--team` or `--all-teams`
- `--shared-only` without `--team` or `--all-teams` requires `shared_repo.default_team` or `shared_repo.default_all_teams`
- `--limit` can only be used with `--list`

## Team Naming Rules

Team names may contain only:

- letters
- numbers
- dots
- underscores
- hyphens

The teams directory must be a relative path and cannot contain `.` or `..` components.

## GitHub Integration

Current GitHub integration is checkout-based. ReqBib does not create commits, push changes, or manage authentication on its own.

Requirements:

- `gh` installed
- `gh` authenticated for the target repository
- `git` installed

Behavior:

- if `shared_repo.mode` is `github`, ReqBib clones into `~/.reqbib/repos/<owner>__<repo>`
- managed checkouts are refreshed with `git pull --ff-only`
- refresh runs at most once per `auto_update_interval_minutes`
- set `auto_update_repo` to `false` to disable refresh entirely

## Search Behavior

ReqBib extracts and indexes keywords from:

- domains and subdomains
- URL path segments
- HTTP methods
- header names and values
- other meaningful words in the command

Search is case-insensitive and supports multiple keywords.

Description text is indexed too, so searches can match either the raw command or its optional description.

## Output Format

Read results are grouped by source:

- `Local`
- `Shared / <team>`

Entries are rendered in multiline-safe blocks. If an entry has a description, it is shown after the bracketed index:

```text
=== LOCAL ===

[1] Fetch Octocat profile
curl https://api.github.com/users/octocat

=== SHARED / PLATFORM ===

[1] Platform health check
curl -X POST https://api.example.com/platform/health \
  -H "Authorization: Bearer $TOKEN"
```

When local and shared output are shown together, ReqBib hides local entries whose command text exactly matches one of the displayed shared entries. A summary line reports how many local entries were hidden.

`--list` output is also limited by default. ReqBib shows the first `20` commands unless `default_list_limit` or `--limit` changes that behavior.

## Examples

Local add and search:

```bash
reqbib -a "curl https://api.github.com/users/octocat"
reqbib -a "curl https://api.github.com/users/octocat" --description "Fetch Octocat profile"
reqbib github octocat
```

Default local + default-team search when `shared_repo.default_team` is configured:

```bash
reqbib health
```

Local-only override:

```bash
reqbib --local-only health
```

Shared-only override:

```bash
reqbib --shared-only health
```

Explicit all-team read:

```bash
reqbib --all-teams health
```

Single-team listing:

```bash
reqbib --repo /path/to/shared-reqbib --team platform -l
```

Cross-team listing:

```bash
reqbib --repo /path/to/shared-reqbib --all-teams -l
```

GitHub-configured usage:

```bash
reqbib --team platform -l
```

That last example assumes `~/.reqbib/config.json` already provides a `shared_repo` configuration.
