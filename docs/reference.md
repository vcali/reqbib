# combib Reference

This document covers configuration, CLI parameters, storage modes, and the current GitHub-backed shared repository workflow.

## Storage Modes

### Local mode

Local storage is biblioteca-based:

```text
~/.combib/
  libs/
    curl.json
    git.json
    aws.json
```

Use local mode when you do not pass `--team` or `--all-teams`.

### Shared repository mode

Shared mode keeps ownership by team and organization by biblioteca:

```text
shared-combib/
  teams/
    platform/
      libs/
        curl.json
        aws.json
    payments/
      libs/
        curl.json
```

Shared mode is activated when:

- `--team <team>` is used
- `--all-teams` is used
- or a default shared target is configured and you use default read commands

Repository resolution order:

1. `--repo <path>`
2. `shared_repo.mode = "path"` from config
3. `shared_repo.mode = "github"` from config, which bootstraps a managed local checkout with `gh repo clone`

## Configuration

Default config location:

```text
~/.combib/config.json
```

You can override that with:

```bash
combib --config /path/to/config.json ...
```

### Path mode

```json
{
  "default_biblioteca": "curl",
  "shared_repo": {
    "mode": "path",
    "path": "/Users/alice/src/shared-combib",
    "teams_dir": "teams"
  }
}
```

### GitHub mode

```json
{
  "default_biblioteca": "curl",
  "shared_repo": {
    "mode": "github",
    "github_repo": "acme/shared-combib",
    "teams_dir": "teams",
    "default_team": "platform",
    "auto_update_repo": true,
    "auto_update_interval_minutes": 15
  },
  "default_list_limit": 20
}
```

Supported top-level keys:

- `default_biblioteca`: optional default biblioteca for normal reads and writes. If omitted, `combib` falls back to the built-in `default` biblioteca
- `shared_repo`: shared repository configuration
- `default_list_limit`: optional default limit for `--list`. `0` means unlimited. If omitted, `combib` defaults to `20`

Supported keys inside `shared_repo`:

- `mode`: must be `path` or `github`
- `path`: required for `mode = "path"`
- `github_repo`: required for `mode = "github"`, in `<owner>/<repo>` format
- `teams_dir`: relative path inside the repository that contains team folders. Defaults to `teams`
- `default_team`: optional default shared team for non-team read commands
- `default_all_teams`: optional shared read default for non-team read commands. Defaults to `false`
- `auto_update_repo`: GitHub mode only. Defaults to `true`
- `auto_update_interval_minutes`: GitHub mode only. Defaults to `15` and must be greater than `0`

Validation rules:

- `mode = "path"` requires `path` and rejects `github_repo`, `auto_update_repo`, and `auto_update_interval_minutes`
- `mode = "github"` requires `github_repo` and rejects `path`
- `default_team` and `default_all_teams = true` cannot be configured together
- `default_biblioteca` must use the normal biblioteca naming rules
- flat legacy config keys such as `github_repo`, `shared_repo_path`, `teams_dir`, and `auto_update_repo` at the top level are rejected

Precedence:

1. CLI arguments
2. Config file
3. Built-in defaults

## CLI Parameters

### Core operations

- `-a`, `--add <COMMAND>`: add a command to the active storage target
- `--description <TEXT>`: optional brief description for `--add`
- `-b`, `--biblioteca <NAME>`: active biblioteca
- `--create-biblioteca <NAME>`: create a new biblioteca in the active local or team-scoped target
- `--list-bibliotecas`: list available bibliotecas in the active local or shared scope
- `-l`, `--list`: list commands in the active biblioteca
- `<keywords...>`: search for commands by keyword in the active biblioteca

### Shared repository options

- `--repo <PATH>`: path to a shared repository checkout
- `--team <TEAM>`: target a single team folder
- `--teams-dir <PATH>`: relative path to the teams directory within the repo
- `--all-teams`: list or search across every team in the same biblioteca
- `--local-only`: limit default list/search commands to local storage
- `--shared-only`: limit default list/search commands to shared storage
- `--limit <COUNT>`: limit how many commands are shown with `--list`. `0` means unlimited

### Configuration option

- `--config <PATH>`: use a non-default `combib` config file

## Biblioteca Rules

Biblioteca names may contain only:

- letters
- numbers
- dots
- underscores
- hyphens

`combib` always resolves an active biblioteca for add, list, and search:

- CLI `-b` / `--biblioteca`
- `default_biblioteca` from config
- built-in fallback: `default`

`--create-biblioteca <NAME>` creates the requested biblioteca file explicitly and exits. If the biblioteca already exists, `combib` reports that and does not overwrite it.

`--list-bibliotecas` does not resolve an active biblioteca. It lists biblioteca names for the selected scope:

- no shared flags: local bibliotecas, plus configured default shared scope if one applies
- `--team <TEAM>`: bibliotecas for that team only
- `--all-teams`: bibliotecas grouped by team
- `--local-only`: local bibliotecas only
- `--shared-only`: shared bibliotecas for the configured default shared scope

## Shared Mode Rules

- `--team` and `--all-teams` cannot be used together
- `--local-only` and `--shared-only` cannot be used together
- `--all-teams` is read-only
- `--all-teams` cannot be used with `--add`
- `--local-only` and `--shared-only` are read-only controls and cannot be used with `--add`
- `--local-only` and `--shared-only` cannot be used with `--team` or `--all-teams`
- `--shared-only` without `--team` or `--all-teams` requires `shared_repo.default_team` or `shared_repo.default_all_teams`
- `--repo` and `--teams-dir` may be used for default read commands without `--team`
- `--repo` and `--teams-dir` still require `--team` for write commands
- `--description` can only be used with `--add`
- `--limit` can only be used with `--list`
- `--biblioteca` cannot be used with `--list-bibliotecas`
- `--list-bibliotecas` cannot be combined with `--add`, `--list`, `--create-biblioteca`, `--description`, `--limit`, or search keywords

Team names follow the same character rules as bibliotecas.

The teams directory must be a relative path and cannot contain `.` or `..` components.

## GitHub Integration

Current GitHub integration is checkout-based. `combib` does not create commits, push changes, or manage authentication on its own.

Requirements:

- `gh` installed
- `gh` authenticated for the target repository
- `git` installed

Behavior:

- if `shared_repo.mode` is `github`, `combib` clones into `~/.combib/repos/<owner>__<repo>`
- managed checkouts are refreshed with `git pull --ff-only`
- refresh runs at most once per `auto_update_interval_minutes`
- set `auto_update_repo` to `false` to disable refresh entirely

## Search Behavior

`combib` extracts and indexes keywords from:

- domains and subdomains
- URL path segments
- HTTP methods
- header names and values
- other meaningful words in the command
- description text

Search is case-insensitive and supports multiple keywords.

For non-HTTP commands, keyword extraction falls back to generic tokenization.

## Output Format

Read results are grouped by source and biblioteca:

- `Local / <biblioteca>`
- `Shared / <team> / <biblioteca>`

Entries are rendered in multiline-safe blocks. If an entry has a description, it is shown after the bracketed index:

```text
=== LOCAL / CURL ===

[1] Fetch Octocat profile
curl https://api.github.com/users/octocat

=== SHARED / PLATFORM / CURL ===

[1] Platform health check
curl -X POST https://api.example.com/platform/health \
  -H "Authorization: Bearer $TOKEN"
```

When local and shared output are shown together, `combib` hides local entries whose command text exactly matches one of the displayed shared entries. A summary line reports how many local entries were hidden.

`--list` output is limited by default. `combib` shows the first `20` commands unless `default_list_limit` or `--limit` changes that behavior.

`--list-bibliotecas` prints one name per line inside source-grouped sections. Team-wide output uses `Shared / <team>` headers because biblioteca names are the payload.

## Examples

Local add and search:

```bash
combib -b curl -a "curl https://api.github.com/users/octocat"
combib -b curl -a "curl https://api.github.com/users/octocat" --description "Fetch Octocat profile"
combib -b curl github octocat
```

Default biblioteca search when `default_biblioteca` is configured:

```bash
combib github octocat
```

Built-in fallback without config:

```bash
combib --add "curl https://example.com/health"
combib -l
```

Create a biblioteca explicitly:

```bash
combib --create-biblioteca git
combib --repo /path/to/shared-combib --team platform --create-biblioteca aws
```

List available bibliotecas:

```bash
combib --list-bibliotecas
combib --repo /path/to/shared-combib --team platform --list-bibliotecas
combib --repo /path/to/shared-combib --all-teams --list-bibliotecas
```

Local-only override:

```bash
combib --local-only -b curl health
```

Shared-only override:

```bash
combib --shared-only -b curl health
```

Explicit all-team read:

```bash
combib --all-teams -b curl health
```

Single-team listing:

```bash
combib --repo /path/to/shared-combib --team platform -b curl -l
```
