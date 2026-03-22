# reqbib

ReqBib is a CLI for storing, searching, and sharing `curl` commands.

The name comes from **Requests Biblioteca**: a library of useful HTTP requests for individuals and teams.

## Highlights

- Store `curl` commands locally in `~/.reqbib/commands.json`
- Add an optional short description to each stored command
- Import commands from shell history with `-i`
- Search by extracted keywords instead of exact text only
- Use a shared team repository layout with GitHub-backed checkouts
- Add a default shared team so normal search and list commands can include local plus your team
- Search across all teams in a shared repository with `--all-teams`

## Quick Start

Add a command locally:

```bash
reqbib -a "curl -I https://api.github.com/users/octocat"
```

Add a command with a short description:

```bash
reqbib -a "curl -I https://api.github.com/users/octocat" \
  --description "Fetch the Octocat profile headers"
```

Search locally:

```bash
reqbib github octocat
```

If `shared_repo.default_team` is configured, that default search includes local commands plus your team. Use `--local-only`, `--shared-only`, or `--all-teams` when you want a different scope.

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

Default local-plus-team output is grouped by source and preserves multiline commands:

```text
=== LOCAL ===

[1] Fetch Octocat profile
curl https://api.github.com/users/octocat

=== SHARED / PLATFORM ===

[1] Platform health check
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
    "default_team": "platform",
    "auto_update_repo": true,
    "auto_update_interval_minutes": 15
  },
  "default_list_limit": 20
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
