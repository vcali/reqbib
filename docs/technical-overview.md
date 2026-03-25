# combib Technical Overview

This document is the maintainer-oriented overview of the current code structure and runtime behavior.

## Code Layout

The application is split into focused modules:

- [`src/main.rs`](../src/main.rs): binary entrypoint
- [`src/lib.rs`](../src/lib.rs): crate wiring and public `run()` entry
- [`src/app.rs`](../src/app.rs): CLI dispatch and output flow
- [`src/cli.rs`](../src/cli.rs): `clap` command definition
- [`src/config.rs`](../src/config.rs): config loading, biblioteca resolution, shared-storage resolution, path validation
- [`src/database.rs`](../src/database.rs): stored command model and JSON persistence
- [`src/github.rs`](../src/github.rs): managed GitHub checkout bootstrap and refresh logic
- [`src/keywords.rs`](../src/keywords.rs): keyword extraction and regex reuse

## Runtime Flow

At a high level, execution is:

1. Build and parse CLI arguments.
2. Load config from `~/.combib/config.json` or `--config`.
3. Resolve the active biblioteca from `-b` / `--biblioteca`, `default_biblioteca`, or the built-in `default` fallback.
4. Resolve local or shared storage context from the nested `shared_repo` config or CLI overrides.
5. For GitHub-backed shared mode, ensure a local checkout exists and refresh it if due.
6. Execute one of the user operations:
   - add
   - create biblioteca
   - list bibliotecas
   - list
   - search
7. Persist updated JSON if the operation mutates storage.

## Storage Model

`combib` uses JSON files and an explicit biblioteca-per-file model.

Local storage:

```text
~/.combib/libs/<biblioteca>.json
```

Shared storage:

```text
<repo>/<teams_dir>/<team>/libs/<biblioteca>.json
```

Each entry stores:

- the original command string
- an optional short description
- the extracted keyword list

## Search Indexing

Search works by precomputing keywords when commands are added.

Current indexing behavior:

- regexes are compiled once and reused
- description text is folded into the stored keyword set when present
- stored keywords are normalized to lowercase
- search keywords are normalized once per query
- fallback substring matching checks the full command text and the optional description
- HTTP commands keep protocol-aware indexing, while non-HTTP commands rely on generic tokenization

This is still a simple in-memory scan over JSON-backed records. It is acceptable for the current scale, but larger shared repositories may eventually need a different storage or indexing strategy.

## GitHub Integration Model

Current GitHub support is intentionally narrow:

- repository selection comes from CLI or `shared_repo` config
- bootstrap uses `gh repo clone`
- refresh uses `git pull --ff-only`
- refresh state is tracked in `~/.combib/state`
- refresh cadence is configurable with `shared_repo.auto_update_interval_minutes`

`combib` does not yet:

- commit
- push
- resolve merge conflicts
- enforce org or team permissions beyond repository layout

## Read Scope

Default read commands can operate in these modes:

- `local`
- `local + default team`
- `local + all teams`
- `shared only` via the configured default shared target

Current behavior:

- the active biblioteca is CLI-selected, config-backed, or falls back to the built-in `default`
- `--create-biblioteca` initializes a biblioteca file explicitly rather than waiting for the first `--add`
- `--list-bibliotecas` skips active-biblioteca resolution and instead enumerates biblioteca files in the selected scope
- if `shared_repo.default_team` is configured, non-team list/search defaults to local plus that team
- if `shared_repo.default_all_teams` is `true`, non-team list/search defaults to local plus all teams
- otherwise non-team list/search defaults to local only
- `--local-only` and `--shared-only` override that behavior
- `--team` and `--all-teams` stay explicit shared-only modes
- local entries that exactly duplicate displayed shared entries are hidden from the default combined output
- `--list` uses a default result cap unless `default_list_limit` or `--limit` overrides it
- `--list-bibliotecas` is uncapped and groups names by local/shared source, or by team when `--all-teams` is used
- output uses plain `=== ... ===` section banners with multiline-safe entry blocks, and descriptions render inline after the bracketed index

## Deliberate Product Constraints

The current product direction is intentionally opinionated:

- commands are curated manually rather than imported from shell history
- bibliotecas are the organization boundary; free-form tags are not part of the model
- shared storage remains team-based to keep ownership simple

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
