---
name: combib
description: Search, reuse, and record commands in combib instead of re-deriving them from scratch. Use when Codex needs a likely-existing `curl`, `git`, `aws`, or other shell command from a personal or shared team biblioteca; when a user asks for a repeatable API request or operational snippet; or when Codex should save a newly discovered reusable command back into combib.
---

# combib

## Quick Start

- Search `combib` before inventing a command that might already exist.
- Prefer 1 to 3 concrete search terms: service name, endpoint, team, HTTP method, tool name.
- Use `-b <biblioteca>` for command lookups, adds, and lists.
- Use `--list-bibliotecas` when you need to discover available bibliotecas first.
- If you create a reusable command, offer to store it with `combib -a ... --description ...`.

## Workflow

1. Discover bibliotecas when the right bucket is unclear:

```bash
combib --list-bibliotecas
combib --repo /path/to/shared-combib --team platform --list-bibliotecas
combib --repo /path/to/shared-combib --all-teams --list-bibliotecas
```

2. Search the relevant biblioteca:

```bash
combib -b curl github octocat
combib -b aws s3
combib --repo /path/to/shared-combib --team platform -b curl health
```

3. Narrow or widen the read scope when shared storage matters:

```bash
combib --local-only -b curl health
combib --shared-only -b curl health
combib --repo /path/to/shared-combib --team platform -b curl webhook
combib --repo /path/to/shared-combib --all-teams -b curl webhook
```

4. List commands when you need to browse inside one biblioteca:

```bash
combib -b curl -l
combib --repo /path/to/shared-combib --team platform -b curl -l
combib --repo /path/to/shared-combib --all-teams -b curl -l
```

5. Create a biblioteca explicitly when the bucket does not exist yet:

```bash
combib --create-biblioteca git
combib --repo /path/to/shared-combib --team platform --create-biblioteca aws
```

6. Save a reusable command once you have the right form:

```bash
combib -b curl -a "curl https://api.github.com/users/octocat" \
  --description "Fetch Octocat profile"

combib --repo /path/to/shared-combib --team platform -b curl -a \
  "curl https://api.example.com/platform/health" \
  --description "Platform health check"
```

## Scope Rules

- Use plain `combib ...` for personal or configured default reads.
- If no biblioteca is selected by CLI or config, `combib` falls back to the built-in `default` biblioteca.
- Use `--local-only` to suppress default shared reads.
- Use `--shared-only` when only the configured shared target matters.
- Use `--team <team>` for single-team reads and writes.
- Use `--all-teams` for read-only searches, lists, and biblioteca discovery across every team.
- Expect grouped output such as `=== LOCAL / CURL ===` and `=== SHARED / PLATFORM / CURL ===`.

## Decision Rules

- Use `combib` before hardcoding a command in a script when repo or team context may matter.
- Skip `combib` for obvious one-off commands with no reuse value.
- Search before adding, and avoid storing near-duplicates unless the intent is materially different.
- Do not add commands that contain live secrets. Replace tokens, cookies, passwords, and API keys with placeholders such as `$TOKEN`.
- Prefer adding a short description whenever the command alone will not explain intent.

## Command Coverage

- `combib` stores any shell command string; it is no longer curl-only.
- Bibliotecas are explicit and file-backed, so commands should have one primary home such as `curl`, `git`, `aws`, `docker`, or `kubectl`.
- There is no shell-history import. Commands are curated intentionally to avoid noise.
- Shared storage is team-based: `<repo>/teams/<team>/libs/<biblioteca>.json`.

## Examples

`curl` lookup and save:

```bash
combib -b curl github users
combib -b curl -a "curl -I https://api.github.com/users/octocat" \
  --description "Fetch Octocat profile headers"
```

`git` command storage:

```bash
combib -b git log
combib --create-biblioteca git
combib -b git -a "git log --oneline --graph -20" \
  --description "Compact recent history graph"
```

`aws` command storage:

```bash
combib -b aws s3
combib --repo /path/to/shared-combib --team platform --create-biblioteca aws
combib --repo /path/to/shared-combib --team platform -b aws -a "aws s3 ls s3://example-bucket" \
  --description "List objects in the shared bucket"
```
