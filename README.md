# combib

`combib` is a CLI for storing, searching, and sharing reusable shell commands.

The name comes from **Command Biblioteca**. It intentionally does not include the word `library`, to avoid confusion with software/code libraries.

## Highlights

- Store any command in an explicit biblioteca such as `curl`, `git`, `aws`, or `kubectl`
- Local storage lives under `~/.combib/libs/<biblioteca>.json`
- Shared storage stays team-based under `<repo>/teams/<team>/libs/<biblioteca>.json`
- Use `-b` / `--biblioteca` to keep reads and writes scoped and tidy
- Search by extracted keywords instead of exact text only
- Use a shared team repository layout with optional GitHub-backed checkouts
- No shell-history import by design; commands are intentionally curated to avoid noise

## Quick Start

Add a command locally:

```bash
combib -b curl -a "curl -I https://api.github.com/users/octocat"
```

Add a command with a short description:

```bash
combib -b git -a "git log --oneline --graph -20" \
  --description "Compact recent history graph"
```

Search within a biblioteca:

```bash
combib -b curl github octocat
combib -b aws s3
```

List a biblioteca:

```bash
combib -b curl -l
```

If `default_biblioteca` is configured, you can omit `-b` for normal reads and writes.

## Team Usage

Shared storage keeps ownership at the team level and organization at the biblioteca level:

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

Basic team-scoped usage:

```bash
combib --repo /path/to/shared-combib --team platform -b curl -a \
  "curl https://api.example.com/platform/health"

combib --repo /path/to/shared-combib --team platform -b curl -l
```

Cross-team search within one biblioteca:

```bash
combib --repo /path/to/shared-combib --all-teams -b curl stripe webhook
```

Default local-plus-team output is grouped by source:

```text
=== LOCAL / CURL ===

[1] Fetch Octocat profile
curl https://api.github.com/users/octocat

=== SHARED / PLATFORM / CURL ===

[1] Platform health check
curl -X POST https://api.example.com/platform/health \
  -H "Authorization: Bearer $TOKEN"
```

## Config

Minimal GitHub-backed config:

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

GitHub-backed shared usage requires:

- `gh` installed and authenticated
- `git` available locally

## Documentation

- Detailed CLI and config reference: [`docs/reference.md`](docs/reference.md)
- Technical overview and code structure: [`docs/technical-overview.md`](docs/technical-overview.md)

## Releases

Pushes to `main` publish a GitHub Release automatically.

- Tags use the format `v<crate-version>-build.<run_number>`
- Release assets currently include:
  - `combib-x86_64-unknown-linux-gnu.tar.gz`
  - `combib-x86_64-apple-darwin.tar.gz`
  - `combib-aarch64-apple-darwin.tar.gz`
  - matching `.sha256` checksum files

## Sensitive Data

`combib` stores commands as provided. If a command contains live tokens, cookies, or other credentials, shared repository mode can expose them to teammates or commit history. Secret detection and redaction are still planned, not implemented.

## Development

Build:

```bash
cargo build
```

Run locally during development:

```bash
cargo run -- -b curl -l
```
