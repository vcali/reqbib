# AGENTS.md - AI Agent Guidelines for ReqBib

## Overview
This file contains instructions and guidelines for AI agents working on the ReqBib project. All agents should follow these rules when making changes to ensure code quality and consistency.

## Project Context and Planning

**MANDATORY: Read and use the DEVELOPMENT_PLAN.md file**

Before making any changes, agents MUST:

1. **Read `DEVELOPMENT_PLAN.md`** to understand:
   - Current project status and implemented features
   - Future expansion plans and roadmap
   - Technical architecture decisions
   - Known limitations and constraints
   - Current defaults for shared repository config, read scope, and output behavior

2. **Update `DEVELOPMENT_PLAN.md`** when making changes:
   - Mark features as completed when implementing them
   - Add new features to the appropriate sections
   - Update implementation status and examples
   - Maintain accurate documentation of the project state
   - Keep it as a local working document only; it is gitignored and must not be committed

3. **Align changes with the plan**:
   - Ensure new features fit within the project's vision
   - Follow established architectural patterns
   - Consider how changes affect future planned features

### Planning and Documentation Notes
- `DEVELOPMENT_PLAN.md` is intentionally local-only and ignored by Git
- Detailed user-facing CLI and config documentation lives in `docs/reference.md`
- Maintainer-oriented architecture notes live in `docs/technical-overview.md`
- Shared repository config uses a nested `shared_repo` object rather than flat top-level keys

## Required Validation Before Commits

**MANDATORY: Every change made by an agent MUST be validated before committing by running:**

1. **Tests** - Ensure all functionality works
   ```bash
   cargo test
   ```

2. **Formatting** - Ensure code follows Rust formatting standards
   ```bash
   cargo fmt --check
   ```

3. **Linting** - Ensure code passes clippy with no warnings
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

4. **Documentation** - Ensure documentation builds without errors
   ```bash
   cargo doc --no-deps --document-private-items
   ```

## Pre-Commit Workflow

When making any code changes, agents MUST follow this workflow:

1. **Make the changes** to source files
2. **Run validation checks** (tests, formatting, linting, docs)
3. **Fix any issues** found during validation
4. **Re-run validation** to confirm fixes
5. **Only then commit and push** the changes

### Example Validation Sequence
```bash
# Run all validation checks
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --document-private-items

# If formatting issues found, fix them
cargo fmt

# Re-run checks to confirm everything passes
cargo fmt --check
cargo test
```

## Project-Specific Guidelines

### Code Style
- Follow Rust standard formatting (enforced by `rustfmt.toml`)
- Use clippy suggestions to improve code quality
- Maintain comprehensive test coverage
- Write clear, descriptive commit messages

### Testing Requirements
- All new functionality MUST have corresponding tests
- Maintain the current test coverage level (100% pass rate)
- Use integration tests for CLI functionality
- Use unit tests for internal logic

### Git Workflow
- Create feature branches for new functionality
- Use descriptive branch names (e.g., `feature/list-filtering`)
- Write clear commit messages explaining the change
- Ensure CI passes before merging

### Documentation
- Update `DEVELOPMENT_PLAN.md` when adding new features
- Ensure code documentation is complete and accurate
- Update usage examples when CLI interface changes

## CI/CD Integration

The project uses GitHub Actions that run the same validation checks. By following the pre-commit validation workflow above, agents ensure that:

- ✅ CI builds will pass
- ✅ Code quality standards are maintained  
- ✅ No regressions are introduced
- ✅ The project remains stable and reliable

## Common Validation Failures and Solutions

### Formatting Issues
**Problem:** `cargo fmt --check` fails
**Solution:** Run `cargo fmt` to auto-fix formatting

### Clippy Warnings
**Problem:** `cargo clippy` reports warnings
**Solution:** Fix warnings by following clippy suggestions

### Test Failures  
**Problem:** `cargo test` shows failing tests
**Solution:** Fix the underlying issue causing test failure

### Documentation Errors
**Problem:** `cargo doc` fails to build
**Solution:** Fix documentation syntax or missing docs

## Emergency Procedures

If validation fails and you need to quickly identify the issue:

1. **Check recent changes:** `git diff HEAD~1`
2. **Run individual checks** to isolate the problem
3. **Fix the specific issue** found
4. **Re-run full validation** before committing

## Agent Responsibilities

Every agent working on this repository is responsible for:

- ✅ Following the validation workflow
- ✅ Maintaining code quality standards
- ✅ Not breaking existing functionality
- ✅ Keeping documentation up to date
- ✅ Ensuring CI passes

**Remember: Prevention is better than fixing CI failures later!**
