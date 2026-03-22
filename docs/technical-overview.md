# ReqBib Technical Overview

This document is the maintainer-oriented overview of the current code structure and runtime behavior.

## Code Layout

The application is now split into focused modules:

- [`src/main.rs`](../src/main.rs): binary entrypoint
- [`src/lib.rs`](../src/lib.rs): crate wiring and public `run()` entry
- [`src/app.rs`](../src/app.rs): CLI dispatch and output flow
- [`src/cli.rs`](../src/cli.rs): `clap` command definition
- [`src/config.rs`](../src/config.rs): config loading, shared-storage resolution, path validation
- [`src/database.rs`](../src/database.rs): command storage model and JSON persistence
- [`src/github.rs`](../src/github.rs): managed GitHub checkout bootstrap and refresh logic
- [`src/history.rs`](../src/history.rs): shell history parsing and import
- [`src/keywords.rs`](../src/keywords.rs): keyword extraction and regex reuse

The goal of this split is to keep feature work from accumulating in one large binary file.

## Runtime Flow

At a high level, execution is:

1. Build and parse CLI arguments.
2. Load config from `~/.reqbib/config.json` or `--config`.
3. Resolve local or shared storage context from the nested `shared_repo` config or CLI overrides.
4. For GitHub-backed shared mode, ensure a local checkout exists and refresh it if due.
5. For default read commands, use local-only, local plus the configured default team, or local plus all teams depending on config and CLI overrides.
6. Execute one of the user operations:
   - add
   - import
   - list
   - search
7. Persist updated JSON if the operation mutates storage.

## Storage Model

ReqBib currently uses JSON files.

Local storage:

```text
~/.reqbib/commands.json
```

Shared storage:

```text
<repo>/<teams_dir>/<team>/commands.json
```

Each entry stores:

- the original command string
- an optional short description
- the extracted keyword list

## Search Indexing

Search works by precomputing keywords when commands are added or imported.

Current indexing behavior:

- regexes are compiled once and reused
- description text is folded into the stored keyword set when present
- stored keywords are normalized to lowercase
- search keywords are normalized once per query
- fallback substring matching checks the full command text and the optional description

This is still a simple in-memory scan over JSON-backed records. It is acceptable for the current scale, but larger shared repositories may eventually need a different storage or indexing strategy.

## GitHub Integration Model

Current GitHub support is intentionally narrow:

- repository selection comes from CLI or `shared_repo` config
- bootstrap uses `gh repo clone`
- refresh uses `git pull --ff-only`
- refresh state is tracked in `~/.reqbib/state`
- refresh cadence is configurable with `shared_repo.auto_update_interval_minutes`

ReqBib does not yet:

- commit
- push
- resolve merge conflicts
- enforce org or team permissions beyond repository layout

## History Import

History import currently reads:

- `~/.bash_history`
- `~/.zsh_history`

Implementation details:

- handles zsh timestamp prefixes
- deduplicates imported commands
- tolerates non-UTF-8 history files via lossy decoding

## Tests

The project currently uses:

- unit tests inside the relevant modules
- integration tests in [`tests/integration_tests.rs`](../tests/integration_tests.rs)

Validation standard:

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --document-private-items
```

## Known Gaps

The main planned gaps relevant to maintainers are:

- secret detection and redaction for stored commands
- deletion workflow and command identity model
- Postman and Insomnia importers
- deeper GitHub sync beyond checkout bootstrap and refresh

## Read Scope

Default read commands can operate in these modes:

- `local`
- `local + default team`
- `local + all teams`
- `shared only` via the configured default shared target

Current behavior:

- if `shared_repo.default_team` is configured, non-team list/search defaults to local plus that team
- if `shared_repo.default_all_teams` is `true`, non-team list/search defaults to local plus all teams
- otherwise non-team list/search defaults to local only
- `--local-only` and `--shared-only` override that behavior
- `--team` and `--all-teams` stay explicit shared-only modes
- local entries that exactly duplicate displayed shared entries are hidden from the default combined output
- `--list` uses a default result cap unless `default_list_limit` or `--limit` overrides it
- output uses plain `=== ... ===` section banners with multiline-safe entry blocks, and descriptions render inline after the bracketed index when present
