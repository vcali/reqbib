# reqbib
Reqbib is a CLI tool for managing your curl commands. Built-in bash history auto-discovery, native Github and S3 integration and more.

**ReqBib** stands for "Request Bibliography" - a smart library for your HTTP requests. It's a Rust-based CLI tool that helps you store, search, and manage your curl commands with intelligent keyword extraction and automatic history import.

## Features

- 🔍 **Smart Search** - Find commands using keywords extracted from URLs, domains, headers
- 📚 **Auto-Import** - Automatically discover and import curl commands from bash/zsh history
- 💾 **Local Storage** - Commands stored securely in `~/.reqbib/commands.json`
- 🚫 **Duplicate Prevention** - Automatically prevents storing duplicate commands
- ⚡ **Fast Retrieval** - Quick keyword-based search across your command library

## Usage

### Add a curl command
```bash
reqbib -a "curl -I https://api.github.com/users/octocat"
```

### Search for commands with keywords
```bash
reqbib github api
# Returns: curl -I https://api.github.com/users/octocat
```

### Import commands from shell history
```bash
reqbib -i
```

### List all stored commands
```bash
reqbib
```

## How It Works

ReqBib intelligently extracts keywords from your curl commands:
- **Domain names**: `github.com` → `["github", "com"]`
- **URL paths**: `/api/v1/users` → `["api", "users"]` 
- **HTTP headers**: `Authorization: Bearer` → `["Authorization"]`
- **Meaningful terms**: Filters out common words like "curl", "http"

## Prerequisites

- [Rust](https://rustup.rs/) (1.70 or later)
- Cargo (comes with Rust)

## Building and Running Locally

### Building the Project

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd reqbib
   ```

2. Build the project:
   ```bash
   cargo build
   ```

   For a release build (optimized):
   ```bash
   cargo build --release
   ```

### Running the Application

#### Development Mode
Run directly with cargo:
```bash
cargo run
```

To pass arguments to the application:
```bash
cargo run -- [your-arguments-here]
```

Examples:
```bash
# Add a command
cargo run -- -a "curl -X GET https://api.example.com/data"

# Search for commands
cargo run -- api data

# Import from history
cargo run -- -i
```

#### Using the Built Binary

After building, you can run the binary directly:

**Debug build:**
```bash
./target/debug/reqbib
```

**Release build:**
```bash
./target/release/reqbib
```

### Installing Locally

To install the binary to your local Cargo bin directory:
```bash
cargo install --path .
```

After installation, you can run `reqbib` from anywhere in your terminal.

## Data Storage

Commands are stored in `~/.reqbib/commands.json` as JSON. The file is automatically created on first use.

### Development

#### Running Tests
```bash
cargo test
```

#### Checking Code
```bash
cargo check
```

#### Formatting Code
```bash
cargo fmt
```

#### Linting
```bash
cargo clippy
```
