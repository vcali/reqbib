# shellshelf

`shellshelf` is a CLI for storing, searching, and sharing reusable shell commands.

The name is meant to evoke a shelf of reusable shell commands without borrowing software-library terminology.

## Highlights

- Store any command in an explicit shelf such as `curl`, `git`, `aws`, or `kubectl`
- Local storage lives under `~/.shellshelf/shelves/<shelf>.json`
- Shared storage stays team-based under `<repo>/teams/<team>/shelves/<shelf>.json`
- Use `-s` / `--shelf` to keep reads and writes scoped and tidy
- Fall back to a built-in `default` shelf when neither CLI nor config selects one
- Create shelves explicitly with `--create-shelf <name>`
- Import exported Postman collections with `--import-postman <file>`
- List available shelves with `--list-shelves`
- Search by extracted keywords instead of exact text only
- Use a shared team repository layout with optional GitHub-backed checkouts
- No shell-history import by design; commands are intentionally curated to avoid noise

## Quick Start

Install with Homebrew:

```bash
brew install vcali/tap/shellshelf
```

Add a command locally:

```bash
shellshelf -s curl -a "curl -I https://api.github.com/users/octocat"
```

Add a command with a short description:

```bash
shellshelf -s git -a "git log --oneline --graph -20" \
  --description "Compact recent history graph"
```

Search within a shelf:

```bash
shellshelf -s curl github octocat
shellshelf -s aws s3
```

List a shelf:

```bash
shellshelf -s curl -l
```

List available shelves:

```bash
shellshelf --list-shelves
shellshelf --repo /path/to/shared-shellshelf --team platform --list-shelves
shellshelf --repo /path/to/shared-shellshelf --all-teams --list-shelves
```

If `default_shelf` is configured, you can omit `-s` for normal reads and writes. If it is not configured, `shellshelf` falls back to the built-in `default` shelf.

Create a shelf explicitly:

```bash
shellshelf --create-shelf git
shellshelf --repo /path/to/shared-shellshelf --team platform --create-shelf aws
```

Import an exported Postman collection into a new shelf:

```bash
shellshelf --import-postman ./postman-api.json
shellshelf --target-shelf postman-api-v2 --import-postman ./postman-api.json
shellshelf --repo /path/to/shared-shellshelf --team platform --import-postman ./platform-api.json
```

## Team Usage

Shared storage keeps ownership at the team level and organization at the shelf level:

```text
shared-shellshelf/
  teams/
    platform/
      shelves/
        curl.json
        aws.json
    payments/
      shelves/
        curl.json
```

Basic team-scoped usage:

```bash
shellshelf --repo /path/to/shared-shellshelf --team platform -s curl -a \
  "curl https://api.example.com/platform/health"

shellshelf --repo /path/to/shared-shellshelf --team platform -s curl -l
```

Cross-team search within one shelf:

```bash
shellshelf --repo /path/to/shared-shellshelf --all-teams -s curl stripe webhook
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
  "default_shelf": "curl",
  "shared_repo": {
    "mode": "github",
    "github_repo": "acme/shared-shellshelf",
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
- The Homebrew tap is `vcali/tap`
- Automatic tap updates require the `HOMEBREW_TAP_TOKEN` repository secret
- Release assets currently include:
  - `shellshelf.rb` for tap publication and manual formula use
  - `shellshelf-x86_64-unknown-linux-gnu.tar.gz`
  - `shellshelf-x86_64-apple-darwin.tar.gz`
  - `shellshelf-aarch64-apple-darwin.tar.gz`
  - matching `.sha256` checksum files

## Sensitive Data

`shellshelf` stores commands as provided. If a command contains live tokens, cookies, or other credentials, shared repository mode can expose them to teammates or commit history. Secret detection and redaction are still planned, not implemented.

Postman import has the same caveat. Imported headers or raw bodies are stored as-is.

## Development

Build:

```bash
cargo build
```

Run locally during development:

```bash
cargo run -- -s curl -l
```
