# reqbib

ReqBib is a CLI for storing, searching, and sharing `curl` commands.

The name comes from **Requests Biblioteca**: a library of useful HTTP requests for individuals and teams.

## Highlights

- Store `curl` commands locally in `~/.reqbib/commands.json`
- Import commands from shell history with `-i`
- Search by extracted keywords instead of exact text only
- Use a shared team repository layout with GitHub-backed checkouts
- Search local and shared commands together by default when `shared_repo` is configured
- Search across all teams in a shared repository with `--all-teams`

## Quick Start

Add a command locally:

```bash
reqbib -a "curl -I https://api.github.com/users/octocat"
```

Search locally:

```bash
reqbib github octocat
```

If `shared_repo` is configured, that default search also includes shared team commands unless you pass `--local-only` or `--shared-only`.

List everything:

```bash
reqbib -l
```

Import from shell history:

```bash
reqbib -i
```

## Team Usage

ReqBib can also work against a shared repository with one folder per team:

```text
shared-reqbib/
  teams/
    platform/
      commands.json
    payments/
      commands.json
```

Basic team-scoped usage:

```bash
reqbib --repo /path/to/shared-reqbib --team platform -a \
  "curl https://api.example.com/platform/health"

reqbib --repo /path/to/shared-reqbib --team platform -l
```

Cross-team search:

```bash
reqbib --repo /path/to/shared-reqbib --all-teams stripe webhook
```

Default combined search output is grouped by source and preserves multiline commands:

```text
Local

[1]
curl https://api.github.com/users/octocat

Shared / platform

[1]
curl -X POST https://api.example.com/platform/health \
  -H "Authorization: Bearer $TOKEN"
```

GitHub-backed shared usage requires:

- `gh` installed and authenticated
- `git` available locally

Minimal GitHub-backed config:

```json
{
  "shared_repo": {
    "mode": "github",
    "github_repo": "acme/shared-reqbib",
    "teams_dir": "teams",
    "auto_update_repo": true,
    "auto_update_interval_minutes": 15
  }
}
```

## Documentation

- Detailed CLI and config reference: [`docs/reference.md`](docs/reference.md)
- Technical overview and code structure: [`docs/technical-overview.md`](docs/technical-overview.md)

## Sensitive Data

ReqBib stores commands as provided. If a command contains live tokens, cookies, or other credentials, shared repository mode can expose them to teammates or commit history. Secret detection and redaction are planned but not implemented yet.

## Development

Build:

```bash
cargo build
```

Run locally during development:

```bash
cargo run -- -l
```
