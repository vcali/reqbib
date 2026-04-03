# shellshelf Reference

This document covers configuration, CLI parameters, storage modes, and the current GitHub-backed shared repository workflow.

## Installation

Install with Homebrew:

```bash
brew install vcali/tap/shellshelf
```

The tap formula tracks the Cargo release version plus a Homebrew revision, so merges that publish a new release without bumping `Cargo.toml` still install and upgrade correctly after `brew update`.

On Linux, the project currently publishes a release tarball rather than distro-specific packages:

```bash
VERSION="v0.2.4" # replace with the release you want
curl -LO "https://github.com/vcali/shellshelf/releases/download/${VERSION}/shellshelf-x86_64-unknown-linux-gnu.tar.gz"
curl -LO "https://github.com/vcali/shellshelf/releases/download/${VERSION}/shellshelf-x86_64-unknown-linux-gnu.tar.gz.sha256"
sha256sum --check shellshelf-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf shellshelf-x86_64-unknown-linux-gnu.tar.gz
install -Dm755 shellshelf ~/.local/bin/shellshelf
```

Make sure `~/.local/bin` is on your `PATH`.

## Storage Modes

### Local mode

Local storage is shelf-based:

```text
~/.shellshelf/
  shelves/
    curl.json
    git.json
    aws.json
```

Use local mode when you do not pass `--team` or `--all-teams`.

### Shared repository mode

Shared mode keeps ownership by team and organization by shelf:

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

Shared mode is activated when:

- `--team <team>` is used
- `--all-teams` is used
- or a shared repository is configured and you use default read commands

Repository resolution order:

1. `--repo <path>`
2. `shared_repo.mode = "path"` from config
3. `shared_repo.mode = "github"` from config, which bootstraps a managed local checkout with `gh repo clone`

Quick setup for GitHub-backed shared mode:

```bash
shellshelf --add-repo https://github.com/acme/shared-shellshelf.git
shellshelf --add-repo acme/shared-shellshelf
```

This updates `~/.shellshelf/config.json` (or `--config <PATH>`) to use `shared_repo.mode = "github"` for that repository while preserving unrelated config such as `default_shelf`, `web`, and existing shared-team defaults.

Once a shared repo is configured, default read commands include local shelves plus all teams unless `shared_repo.default_team` narrows that default or you explicitly scope the read with CLI flags.

## Configuration

Default config location:

```text
~/.shellshelf/config.json
```

You can override that with:

```bash
shellshelf --config /path/to/config.json ...
```

### Path mode

```json
{
  "default_shelf": "curl",
  "shared_repo": {
    "mode": "path",
    "path": "/Users/alice/src/shared-shellshelf",
    "teams_dir": "teams"
  }
}
```

### GitHub mode

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

Supported top-level keys:

- `default_shelf`: optional default shelf for writes and shelf-scoped list/search operations. If omitted, `shellshelf` falls back to the built-in `default` shelf when an active shelf is required
- `shared_repo`: shared repository configuration
- `web`: optional web-interface configuration
- `default_list_limit`: optional default limit for `--list`. `0` means unlimited. If omitted, `shellshelf` defaults to `20`

Supported keys inside `web`:

- `port`: optional localhost port for `--web`. Defaults to `4812`
- `theme`: optional web theme. Supported values are `solarized-dark`, `solarized-light`, `giphy`, and `dracula`. Defaults to `dracula`

Supported keys inside `shared_repo`:

- `mode`: must be `path` or `github`
- `path`: required for `mode = "path"`
- `github_repo`: required for `mode = "github"`, in `<owner>/<repo>` format
- `teams_dir`: relative path inside the repository that contains team folders. Defaults to `teams`
- `default_team`: optional default shared team for non-team read commands
- `default_all_teams`: optional explicit shared read default for non-team read commands. This matches the built-in default when a shared repo is configured
- `auto_update_repo`: GitHub mode only. Defaults to `true`
- `auto_update_interval_minutes`: GitHub mode only. Defaults to `15` and must be greater than `0`

Validation rules:

- `mode = "path"` requires `path` and rejects `github_repo`, `auto_update_repo`, and `auto_update_interval_minutes`
- `mode = "github"` requires `github_repo` and rejects `path`
- `default_team` and `default_all_teams = true` cannot be configured together
- `default_shelf` must use the normal shelf naming rules
- flat legacy config keys such as `github_repo`, `shared_repo_path`, `teams_dir`, and `auto_update_repo` at the top level are rejected

Precedence:

1. CLI arguments
2. Config file
3. Built-in defaults

## CLI Parameters

### Core operations

- `--web`: run the localhost web interface
- `--web-port <PORT>`: port for the localhost web interface. Overrides config and otherwise defaults to `4812`
- `-a`, `--add <COMMAND>`: add a command to the active storage target
- `--description <TEXT>`: optional brief description for `--add`
- `--import-postman <PATH>`: import an exported Postman collection JSON into a new shelf
- `--target-shelf <NAME>`: override the new shelf name when importing from Postman
- `-s`, `--shelf <NAME>`: active shelf
- `--create-shelf <NAME>`: create a new shelf in the active local or team-scoped target
- `--list-shelves`: list available shelves in the active local or shared scope
- `-l`, `--list`: list commands in the active shelf
- `<keywords...>`: search for commands by keyword. With `--shelf`, search stays in that shelf; without it, search spans all shelves in the selected read scope

### Shared repository options

- `--repo <PATH>`: path to a shared repository checkout
- `--add-repo <GITHUB_REPO>`: configure the shared GitHub repository in config from a GitHub URL or `owner/repo`
- `--team <TEAM>`: target a single team folder
- `--open-pr`: commit, push, and open a pull request for a shared write operation
- `--base-branch <BRANCH>`: base branch to fetch and rebase onto before `--open-pr`
- `--pr-branch <BRANCH>`: publish branch to create or reuse for `--open-pr`
- `--teams-dir <PATH>`: relative path to the teams directory within the repo
- `--all-teams`: list or search across every team in the same shelf
- `--local-only`: limit default list/search commands to local storage
- `--shared-only`: limit default list/search commands to shared storage
- `--limit <COUNT>`: limit how many commands are shown with `--list`. `0` means unlimited

### Configuration option

- `--config <PATH>`: use a non-default `shellshelf` config file

`--add-repo` is a setup command and must be used on its own.

## Web Interface

`shellshelf --web` runs a localhost-only web interface for interactive HTTP work.

Current behavior:

- binds to `127.0.0.1`
- uses `--web-port <PORT>` when provided, otherwise `web.port`, otherwise `4812`
- applies `web.theme` from config when present, with `dracula` as the default
- reads local shelves plus any shared repository configured through `--repo`, `--teams-dir`, or `shared_repo` config
- renders local shelves and shared team shelves in an expandable tree explorer
- shows all stored commands, but only runs commands that validate as supported curl commands
- loads selected commands into an editable workbench with editable description and command fields
- can create shelves in the visible local or team-scoped shared area
- can save new commands or update the selected command in the current shelf
- displays parsed request method, URL, and request headers next to response headers after a curl run
- previews text responses inline
- previews image and video responses inline when the response content type is previewable
- keeps response bodies ephemeral in memory for the running process

Current curl execution constraints in the web interface:

- only commands whose executable is `curl` are runnable
- commands using output/capture flags that conflict with the web runner, such as `--output`, `--dump-header`, `--include`, `--head`, `--config`, `--write-out`, and related short forms, are rejected
- non-curl commands remain browseable and saveable but are marked non-runnable

## Shelf Rules

Shelf names may contain only:

- letters
- numbers
- dots
- underscores
- hyphens

`shellshelf` always resolves an active shelf for add and `--list`, and also for search when `--shelf` is provided:

- CLI `-s` / `--shelf`
- `default_shelf` from config
- built-in fallback: `default`

When search keywords are provided without `--shelf`, `shellshelf` searches across all shelves in the selected scope:

- default reads: local shelves, plus all shared teams when a shared repo is configured, unless `shared_repo.default_team` narrows the default
- `--team <TEAM>`: every shelf for that team
- `--all-teams`: every shelf for every team
- `--local-only`: local shelves only
- `--shared-only`: every shelf in the default shared scope, which is all teams unless `shared_repo.default_team` narrows it

`--create-shelf <NAME>` creates the requested shelf file explicitly and exits. If the shelf already exists, `shellshelf` reports that and does not overwrite it.

When `--open-pr` is added to a shared `--create-shelf`, `shellshelf` prepares a clean publish branch before writing, then commits the new shelf file, pushes it, and opens a pull request after the write succeeds.

`--import-postman <PATH>` imports an exported Postman Collection v2.1 JSON file into a new shelf. By default the collection name becomes the shelf name. `--target-shelf <NAME>` may be used to override that name.

Import behavior:

- local by default
- shared when `--team <TEAM>` is provided
- `--repo` and `--teams-dir` are allowed for shared imports when paired with `--team`
- import fails if the target shelf already exists
- a shelf-name collision error tells the user to retry with `--target-shelf`
- import errors on invalid JSON, unsupported schema, invalid shelf name, or when every request is unsupported
- import warns explicitly when some requests are skipped
- supported request bodies currently include raw bodies and common `formdata` bodies converted to `curl -F`
- empty text form-data values are preserved instead of being dropped
- `--open-pr` may be combined with shared imports to commit, push, and open a pull request for the imported shelf file

`--list-shelves` does not resolve an active shelf. It lists shelf names for the selected scope:

- no shared flags: local shelves, plus all shared teams when a shared repo is configured, unless `shared_repo.default_team` narrows the default
- `--team <TEAM>`: shelves for that team only
- `--all-teams`: shelves grouped by team
- `--local-only`: local shelves only
- `--shared-only`: shared shelves for the default shared scope, which is all teams unless `shared_repo.default_team` narrows it

## Shared Mode Rules

- `--team` and `--all-teams` cannot be used together
- `--local-only` and `--shared-only` cannot be used together
- `--web-port` can only be used with `--web`
- `--web` cannot be combined with `--add`, `--list`, `--list-shelves`, `--create-shelf`, `--import-postman`, `--description`, `--limit`, `--shelf`, `--team`, `--all-teams`, `--local-only`, `--shared-only`, or search keywords
- `--all-teams` is read-only
- `--all-teams` cannot be used with `--add`
- `--all-teams` cannot be used with `--import-postman`
- `--open-pr` can only be used with shared `--add`, `--create-shelf`, or `--import-postman` operations
- `--open-pr` requires `--team`
- `--base-branch` and `--pr-branch` require `--open-pr`
- `--local-only` and `--shared-only` are read-only controls and cannot be used with `--add`
- `--local-only` and `--shared-only` are read-only controls and cannot be used with `--import-postman`
- `--local-only` and `--shared-only` cannot be used with `--team` or `--all-teams`
- `--repo` and `--teams-dir` may be used for default read commands without `--team`
- `--repo` and `--teams-dir` still require `--team` for write commands
- `--description` can only be used with `--add`
- `--limit` can only be used with `--list`
- `--shelf` cannot be used with `--list-shelves`
- `--list-shelves` cannot be combined with `--add`, `--list`, `--create-shelf`, `--description`, `--limit`, or search keywords
- `--shelf` cannot be used with `--import-postman`; use `--target-shelf` instead
- `--import-postman` cannot be combined with `--add`, `--list`, `--list-shelves`, `--create-shelf`, `--description`, `--limit`, or search keywords

Team names follow the same character rules as shelves.

The teams directory must be a relative path and cannot contain `.` or `..` components.

## GitHub Integration

Current GitHub integration is checkout-based for reads and PR-based for optional shared writes. `shellshelf` still relies on local `git` and `gh` authentication rather than managing credentials on its own.

Requirements:

- `gh` installed
- `gh` authenticated for the target repository
- `git` installed
- a clean shared checkout before `--open-pr`

Behavior:

- if `shared_repo.mode` is `github`, `shellshelf` clones into `~/.shellshelf/repos/<owner>__<repo>`
- managed checkouts are refreshed with `git pull --ff-only`
- refresh runs at most once per `auto_update_interval_minutes`
- set `auto_update_repo` to `false` to disable refresh entirely
- `--open-pr` is available on shared `--add`, `--create-shelf`, and `--import-postman`
- before the write, `shellshelf` checks that the checkout is clean, fetches `origin/<base-branch>`, switches to the requested or generated publish branch, and rebases onto the chosen base branch
- if `--base-branch` is omitted, `shellshelf` prefers `origin/HEAD` and falls back to `main` or `master`
- if `--pr-branch` is omitted, `shellshelf` reuses the current non-base branch when possible; otherwise it creates a branch named like `shellshelf/<team>-<shelf>`
- after the write, `shellshelf` stages the changed shelf JSON, commits it, pushes it with `git push --set-upstream origin <branch>`, and opens a pull request with `gh pr create`
- if the write is a no-op, `shellshelf` skips commit, push, and PR creation

## Search Behavior

`shellshelf` extracts and indexes keywords from:

- domains and subdomains
- URL path segments
- HTTP methods
- header names and values
- other meaningful words in the command
- description text

## Web Response Rendering

The web interface inspects the captured response content type after a curl run:

- `text/*`, JSON, XML, JavaScript, and other text-like responses render as text
- `image/*` responses render in an inline image preview
- `video/*` responses render in an inline video preview
- other binary responses are captured, but fall back to metadata-only display instead of a forced inline preview
- shelf names for the shelf currently being evaluated

Search is case-insensitive and supports multiple keywords. Each keyword may match command text, extracted command keywords, description text, or the candidate result's shelf name. Existing AND semantics still apply across the full query.

For non-HTTP commands, keyword extraction falls back to generic tokenization.

## Output Format

Read results are grouped by source and shelf:

- `Local / <shelf>`
- `Shared / <team> / <shelf>`

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

When local and shared output are shown together, `shellshelf` hides local entries whose command text exactly matches one of the displayed shared entries. A summary line reports how many local entries were hidden.

`--list` output is limited by default. `shellshelf` shows the first `20` commands unless `default_list_limit` or `--limit` changes that behavior.

`--list-shelves` prints one name per line inside source-grouped sections. Team-wide output uses `Shared / <team>` headers because shelf names are the payload.

## Examples

Local add and search:

```bash
shellshelf -s curl -a "curl https://api.github.com/users/octocat"
shellshelf -s curl -a "curl https://api.github.com/users/octocat" --description "Fetch Octocat profile"
shellshelf -s curl github octocat
```

Default all-shelf search without `--shelf`:

```bash
shellshelf github octocat
shellshelf media upload
```

Built-in fallback without config:

```bash
shellshelf --add "curl https://example.com/health"
shellshelf -l
```

Create a shelf explicitly:

```bash
shellshelf --create-shelf git
shellshelf --repo /path/to/shared-shellshelf --team platform --create-shelf aws
```

Import a Postman collection:

```bash
shellshelf --import-postman ./postman-api.json
shellshelf --target-shelf curl --import-postman ./postman-api.json
shellshelf --repo /path/to/shared-shellshelf --team platform --import-postman ./platform-api.json
```

List available shelves:

```bash
shellshelf --list-shelves
shellshelf --repo /path/to/shared-shellshelf --team platform --list-shelves
shellshelf --repo /path/to/shared-shellshelf --all-teams --list-shelves
```

Local-only override:

```bash
shellshelf --local-only -s curl health
```

Shared-only override:

```bash
shellshelf --shared-only -s curl health
```

Explicit all-team read:

```bash
shellshelf --all-teams -s curl health
```

Single-team listing:

```bash
shellshelf --repo /path/to/shared-shellshelf --team platform -s curl -l
```
