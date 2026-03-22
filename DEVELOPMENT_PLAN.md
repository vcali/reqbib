# ReqBib - Curl Command Management CLI

## Project Overview
ReqBib is a Rust-based CLI tool designed to facilitate the management of curl commands. The name suggests "Request Bibliography" - a library of your HTTP requests.

**Open Source Project** - ReqBib will be released as an open source project with the following distribution strategy:
- **Binary Releases**: Pre-built binaries for Linux, macOS, and Windows available via GitHub Releases
- **Package Manager Support**: Installation via Homebrew for macOS/Linux users
- **Source Distribution**: Full source code available for compilation on any supported platform
- **Cross-platform Compatibility**: Native binaries optimized for each target platform

## Current Implementation Status ✅

### Core Features Implemented
1. **Built in Rust** - Complete implementation using modern Rust libraries
2. **Local Storage** - Commands stored in `~/.reqbib/commands.json` as JSON
3. **Keyword Search** - Smart search functionality with extracted keywords
4. **Manual Addition** - Add commands via `reqbib -a <curl_command>`
5. **History Import** - Automatically import from bash/zsh history with `reqbib -i`
6. **List Commands** - Display all stored commands with `reqbib -l` or `--list`
7. **Filter Commands** - Filter the command list using multiple keywords (e.g., `reqbib -l github api`)
8. **Help by Default** - Show help output when no parameters are provided
9. **Comprehensive Test Suite** - Full unit and integration test coverage

### Technical Architecture
- **Storage**: JSON file at `~/.reqbib/commands.json`
- **CLI Framework**: `clap` v4.0 with derive features
- **Serialization**: `serde` + `serde_json`
- **Regex Processing**: Smart keyword extraction from URLs, domains, paths, headers
- **Regex Reuse**: Shared static regex compilation for keyword extraction and history parsing
- **Search Indexing**: Lowercased keyword matching to reduce repeated normalization during searches
- **History Resilience**: Lossy history file decoding to tolerate non-UTF-8 shell history entries
- **Cross-shell Support**: Imports from both `.bash_history` and `.zsh_history`
- **Testing**: 35 tests (19 unit + 16 integration) with 100% pass rate

### CLI Interface Updates ✅
**Recent Changes:**
- **Default Behavior**: Running `reqbib` without arguments now shows help instead of listing commands
- **New List Option**: Added `-l`/`--list` flag to explicitly list all stored commands
- **Improved UX**: More intuitive user experience with better discoverability
- **Performance Cleanup**: Reused compiled regexes, normalized keyword indexing, and reduced duplicate-check overhead during history import
- **History Import Fix**: Shell history import now handles non-UTF-8 history files instead of skipping them

### Smart Keyword Extraction
The tool automatically extracts keywords from:
- Domain names and subdomains (e.g., "api.github.com" → ["api", "github", "com"])
- URL path segments (e.g., "/user/repos" → ["user", "repos"])
- HTTP methods (e.g., "-X POST" → ["POST"])
- HTTP headers (e.g., "Authorization: Bearer token" → ["Authorization", "Bearer", "token"])
- General meaningful words (excluding common terms like "curl", "http", "https", "www")

### Testing Implementation ✅
**Comprehensive Test Suite:**

**Unit Tests (19 tests):**
- Keyword extraction from various curl command formats
- Database operations (add, bulk add, search, save/load, duplicates)
- History parsing with mocked bash, zsh, and non-UTF-8 history formats
- Search functionality (case-insensitive, partial matches, multi-keyword)
- File I/O operations with temporary directories

**Integration Tests (16 tests):**
- CLI interface testing using `assert_cmd`
- Help system verification
- Command addition and listing functionality
- Search operations with real command execution
- History import with mocked history files
- Edge cases and error handling
- Short and long flag variations

**Testing Dependencies:**
- `tempfile` - Isolated test environments
- `assert_cmd` - CLI command testing
- `predicates` - Output assertions

**Mock Strategy for History Import:**
- Refactored history import to separate parsing logic
- Created mock history files in temporary directories
- Tested both bash and zsh history formats
- Ensured no interference with real user data

### Usage Examples
```bash
# Show help (default behavior)
reqbib

# Add a curl command
reqbib -a "curl -I https://media1.giphy.com/media/123qwe345ert/giphy.webp"

# Search with keywords
reqbib giphy media
# Returns: curl -I https://media1.giphy.com/media/123qwe345ert/giphy.webp

# Import from shell history
reqbib -i

# List all stored commands
reqbib -l
reqbib --list

# Filter commands with keywords
reqbib -l github api
```

## Future Expansion Plans 🚀

### Release & Distribution Strategy 📦
**Open Source Distribution:**
1. **GitHub Releases** - Automated binary releases for each version tag
   - Linux (x86_64, ARM64)
   - macOS (Intel, Apple Silicon)
   - Windows (x86_64)
   
2. **Homebrew Formula** - Package for easy installation on macOS/Linux
   - Create and maintain Homebrew tap: `homebrew-reqbib`
   - Formula for building from source or installing pre-built binaries
   - Integration with GitHub Actions for automated formula updates

3. **Package Managers** (Future consideration)
   - Cargo: `cargo install reqbib`
   - Arch Linux AUR package
   - Debian/Ubuntu .deb packages
   - RPM packages for Red Hat/CentOS/Fedora

4. **CI/CD Pipeline Enhancements**
   - Automated release creation on version tags
   - Cross-compilation for all target platforms
   - Artifact signing for security
   - Homebrew tap updates via GitHub Actions
   - Release notes generation from CHANGELOG.md

### Immediate Next Steps
1. **Add grpcurl support** - Extend beyond HTTP to gRPC requests
2. **Enhanced search** - Fuzzy matching, ranking by relevance
3. **Command organization** - Tags, categories, or folders
4. **Export functionality** - Export commands to various formats

### Completed Optimization Work ✅
1. **Regex compilation reuse** - Eliminated repeated runtime regex compilation on hot paths
2. **Search normalization** - Reduced repeated lowercase conversions during command searches
3. **Bulk import deduplication** - Switched history import deduplication to set-based tracking for better scaling

### CI/CD Optimization Tasks
1. **Windows Build Optimization** - Research faster Windows runners or pre-built Docker images for Rust
   - Consider using `actions/cache` with Windows-specific keys
   - Investigate GitHub's larger Windows runners or self-hosted options
   - Look into pre-built Windows containers with Rust toolchain
   - Alternative: Cross-compile Windows binaries from Linux runners

### Potential Features
1. **Request Templates** - Parameterized requests with variable substitution
2. **Collections** - Group related requests together
3. **Response Storage** - Cache and search through previous responses
4. **Integration** - VS Code extension, shell completions
5. **Collaboration** - Share collections, sync across machines
6. **Analytics** - Track usage patterns, popular endpoints

### Architecture Considerations
- **Plugin System**: Design for extensibility beyond curl/grpcurl
- **Configuration**: User preferences, default behaviors
- **Performance**: Efficient search for large command libraries
- **Cross-platform**: Ensure Windows/Linux compatibility

## Development Notes

### Dependencies
- `clap`: CLI argument parsing with derive macros
- `serde` + `serde_json`: Data serialization
- `dirs`: Cross-platform directory access
- `regex`: Pattern matching for keyword extraction

### Test Dependencies
- `tempfile`: Temporary file/directory creation for isolated tests
- `assert_cmd`: Command-line application testing
- `predicates`: Assertion helpers for test output validation

### Code Structure
- Single binary design for simplicity
- Modular functions for easy extension and testing
- Strong error handling throughout
- Smart duplicate prevention
- Comprehensive test coverage ensuring reliability

### Testing Strategy ✅
**Completed Testing Implementation:**
- **Unit tests** for all core functionality (keyword extraction, database operations, history parsing)
- **Integration tests** for complete CLI interface
- **Mock history files** for safe import testing without affecting user data
- **Isolated environments** using temporary directories
- **Cross-platform path handling** tests
- **Edge case coverage** including error conditions and empty states

### Quality Assurance ✅
- **34 total tests** with 100% pass rate
- **35 total tests** with 100% pass rate
- **Continuous validation** of all functionality
- **Regression protection** for future changes
- **Documentation** through test examples

## Known Limitations
1. **History Format Variations** - Different shell history formats may need handling
2. **Complex Curl Commands** - Very complex multiline commands might need special handling
3. **Performance** - Large history files might slow import process
4. **Keyword Conflicts** - Common words might create noise in search

## Success Metrics
The tool successfully demonstrates:
- ✅ Fast command retrieval by keywords
- ✅ Automatic deduplication
- ✅ Cross-shell history import
- ✅ Clean, intuitive CLI interface
- ✅ Reliable local storage
- ✅ Comprehensive test coverage
- ✅ Robust error handling
- ✅ User-friendly help system

This foundation provides a solid, well-tested base for expansion into a comprehensive request management tool with confidence in reliability and maintainability.
