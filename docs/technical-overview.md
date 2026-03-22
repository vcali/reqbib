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
5. For default read commands, merge local and shared results when `shared_repo` is configured unless scope is overridden by CLI flags or config.
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
- the extracted keyword list

## Search Indexing

Search works by precomputing keywords when commands are added or imported.

Current indexing behavior:

- regexes are compiled once and reused
- stored keywords are normalized to lowercase
- search keywords are normalized once per query
- fallback substring matching still checks the full command text

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

Default read commands can operate in three scopes:

- `local`
- `shared`
- `combined`

Current behavior:

- if `shared_repo` is configured, non-team list/search defaults to `combined`
- `--local-only` and `--shared-only` override that behavior
- `default_read_scope` in config can change the default for non-team reads
- `--team` and `--all-teams` stay explicit shared-only modes
