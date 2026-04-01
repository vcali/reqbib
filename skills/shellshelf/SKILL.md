---
name: shellshelf
description: Search, reuse, and record commands in shellshelf instead of re-deriving them from scratch. Use when Codex needs a likely-existing `curl`, `git`, `aws`, or other shell command from a personal or shared team shelf; when a user asks for a repeatable API request or operational snippet; or when Codex should save a newly discovered reusable command back into shellshelf.
---

# shellshelf

## Quick Start

- Search `shellshelf` before inventing a command that might already exist.
- Prefer 1 to 3 concrete search terms: service name, endpoint, team, HTTP method, tool name.
- Use `-s <shelf>` for command lookups, adds, and lists.
- Use `--target-shelf <name>` when importing a Postman collection into a new shelf with a name override.
- Use `--list-shelves` when you need to discover available shelves first.
- If you create a reusable command, offer to store it with `shellshelf -a ... --description ...`.

## Workflow

1. Discover shelves when the right bucket is unclear:

```bash
shellshelf --list-shelves
shellshelf --repo /path/to/shared-shellshelf --team platform --list-shelves
shellshelf --repo /path/to/shared-shellshelf --all-teams --list-shelves
```

2. Search the relevant shelf:

```bash
shellshelf -s curl github octocat
shellshelf -s aws s3
shellshelf --repo /path/to/shared-shellshelf --team platform -s curl health
```

3. Narrow or widen the read scope when shared storage matters:

```bash
shellshelf --local-only -s curl health
shellshelf --shared-only -s curl health
shellshelf --repo /path/to/shared-shellshelf --team platform -s curl webhook
shellshelf --repo /path/to/shared-shellshelf --all-teams -s curl webhook
```

4. List commands when you need to browse inside one shelf:

```bash
shellshelf -s curl -l
shellshelf --repo /path/to/shared-shellshelf --team platform -s curl -l
shellshelf --repo /path/to/shared-shellshelf --all-teams -s curl -l
```

5. Create a shelf explicitly when the bucket does not exist yet:

```bash
shellshelf --create-shelf git
shellshelf --repo /path/to/shared-shellshelf --team platform --create-shelf aws
```

6. Save a reusable command once you have the right form:

```bash
shellshelf -s curl -a "curl https://api.github.com/users/octocat" \
  --description "Fetch Octocat profile"

shellshelf --repo /path/to/shared-shellshelf --team platform -s curl -a \
  "curl https://api.example.com/platform/health" \
  --description "Platform health check"
```

7. Import an exported Postman collection when a shelf should be seeded from an API client export:

```bash
shellshelf --import-postman ./postman-api.json
shellshelf --target-shelf postman-api-v2 --import-postman ./postman-api.json
shellshelf --repo /path/to/shared-shellshelf --team platform --import-postman ./platform-api.json
```

## Scope Rules

- Use plain `shellshelf ...` for personal or configured default reads.
- If no shelf is selected by CLI or config, `shellshelf` falls back to the built-in `default` shelf.
- Use `--local-only` to suppress default shared reads.
- Use `--shared-only` when only the configured shared target matters.
- Use `--team <team>` for single-team reads and writes.
- Use `--all-teams` for read-only searches, lists, and shelf discovery across every team.
- Expect grouped output such as `=== LOCAL / CURL ===` and `=== SHARED / PLATFORM / CURL ===`.

## Decision Rules

- Use `shellshelf` before hardcoding a command in a script when repo or team context may matter.
- Skip `shellshelf` for obvious one-off commands with no reuse value.
- Search before adding, and avoid storing near-duplicates unless the intent is materially different.
- If a Postman collection name collides with an existing shelf, retry the import with `--target-shelf <name>`.
- Do not add commands that contain live secrets. Replace tokens, cookies, passwords, and API keys with placeholders such as `$TOKEN`.
- Prefer adding a short description whenever the command alone will not explain intent.

## Command Coverage

- `shellshelf` stores any shell command string; it is no longer curl-only.
- Shelves are explicit and file-backed, so commands should have one primary home such as `curl`, `git`, `aws`, `docker`, or `kubectl`.
- There is no shell-history import. Commands are curated intentionally to avoid noise.
- Exported Postman Collection v2.1 JSON imports are supported for creating new shelves.
- Shared storage is team-based: `<repo>/teams/<team>/shelves/<shelf>.json`.

## Examples

`curl` lookup and save:

```bash
shellshelf -s curl github users
shellshelf -s curl -a "curl -I https://api.github.com/users/octocat" \
  --description "Fetch Octocat profile headers"
```

`git` command storage:

```bash
shellshelf -s git log
shellshelf --create-shelf git
shellshelf -s git -a "git log --oneline --graph -20" \
  --description "Compact recent history graph"
```

`aws` command storage:

```bash
shellshelf -s aws s3
shellshelf --repo /path/to/shared-shellshelf --team platform --create-shelf aws
shellshelf --repo /path/to/shared-shellshelf --team platform -s aws -a "aws s3 ls s3://example-bucket" \
  --description "List objects in the shared bucket"
```
