# ReqBib - Curl Command Management CLI

## Project Overview
ReqBib is a Rust-based CLI tool designed to facilitate the management of curl commands. The name suggests "Request Bibliography" - a library of your HTTP requests.

## Current Implementation Status ✅

### Core Features Implemented
1. **Built in Rust** - Complete implementation using modern Rust libraries
2. **Local Storage** - Commands stored in `~/.reqbib/commands.json` as JSON
3. **Keyword Search** - Smart search functionality with extracted keywords
4. **Manual Addition** - Add commands via `reqbib -a <curl_command>`
5. **History Import** - Automatically import from bash/zsh history with `reqbib -i`

### Technical Architecture
- **Storage**: JSON file at `~/.reqbib/commands.json`
- **CLI Framework**: `clap` v4.0 with derive features
- **Serialization**: `serde` + `serde_json`
- **Regex Processing**: Smart keyword extraction from URLs, domains, paths, headers
- **Cross-shell Support**: Imports from both `.bash_history` and `.zsh_history`

### Smart Keyword Extraction
The tool automatically extracts keywords from:
- Domain names and subdomains (e.g., "giphy.com" → ["giphy", "com"])
- URL path segments (e.g., "/api/v1/users" → ["api", "users"])
- HTTP headers (e.g., "Authorization: Bearer" → ["Authorization"])
- General meaningful words (excluding common terms like "curl", "http")

### Usage Examples
```bash
# Add a curl command
reqbib -a "curl -I https://media1.giphy.com/media/123qwe345ert/giphy.webp"

# Search with keywords
reqbib giphy media
# Returns: curl -I https://media1.giphy.com/media/123qwe345ert/giphy.webp

# Import from shell history
reqbib -i

# List all stored commands
reqbib
```

## Future Expansion Plans 🚀

### Immediate Next Steps
1. **Add grpcurl support** - Extend beyond HTTP to gRPC requests
2. **Enhanced search** - Fuzzy matching, ranking by relevance
3. **Command organization** - Tags, categories, or folders
4. **Export functionality** - Export commands to various formats

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

### Code Structure
- Single binary design for simplicity
- Modular functions for easy extension
- Strong error handling throughout
- Smart duplicate prevention

### Testing Strategy
- Unit tests for keyword extraction
- Integration tests for CLI commands  
- Mock history files for import testing
- Cross-platform path handling tests

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

This foundation provides a solid base for expansion into a comprehensive request management tool.