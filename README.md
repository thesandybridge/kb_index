# KB-Index

A command-line tool for indexing and searching local code and documentation using semantic search powered by OpenAI embeddings and ChromaDB.

![License](https://img.shields.io/badge/license-MIT-blue.svg)

## Overview

KB-Index (Knowledge Base Index) is a tool that helps developers create a searchable knowledge base from their local files. It uses OpenAI's text embeddings to create semantic representations of your code and documentation, storing them in a ChromaDB vector database for efficient similarity search.

**Key Features:**
- Index code and documentation files with semantic understanding
- Search your codebase using natural language queries
- Highlight and format search results for easy reading
- Multiple output formats (pretty, JSON, markdown)
- Simple configuration management

## Installation

### Prerequisites

- Rust and Cargo (install via [rustup](https://rustup.rs/))
- An OpenAI API key
- A running ChromaDB instance (local or remote)

### Building from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/kb-index.git
   cd kb-index
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

3. Install the binary (optional):
   ```bash
   cargo install --path .
   ```

## Configuration

KB-Index requires configuration before first use:

### Setting up your OpenAI API Key

You can set your OpenAI API key in one of two ways:

1. Using the config command (recommended):
   ```bash
   kb-index config --set-api-key="your_openai_api_key_here"
   ```

2. Using an environment variable:
   ```bash
   export OPENAI_API_KEY="your_openai_api_key_here"
   ```

### ChromaDB Configuration

By default, KB-Index connects to ChromaDB at `http://localhost:8000`. You can modify this in the config file located at:

- Linux/macOS: `~/.config/kb-index/config.toml`
- Windows: `%APPDATA%\kb-index\config.toml`

Example config.toml:
```toml
chroma_host = "http://localhost:8000"
openai_api_key = "your_openai_api_key_here"
```

You can view your current configuration with:
```bash
kb-index config --show
```

## Usage

### Indexing Files

Index a single file or directory:

```bash
kb-index index /path/to/your/code
```

This will:
1. Recursively find all supported files (md, rs, tsx, ts, js, jsx)
2. Split them into manageable chunks
3. Generate embeddings using OpenAI
4. Store them in ChromaDB

### Searching

Search your indexed files with natural language:

```bash
kb-index query "How does the authentication system work?"
```

Options:
- `--top-k` or `-k`: Number of results to return (default: 5)
- `--format` or `-f`: Output format (options: pretty, json, markdown)

Examples:
```bash
# Get 10 results
kb-index query "How to connect to the database" --top-k 10

# Output in markdown format
kb-index query "Error handling patterns" --format markdown

# Output in JSON format for programmatic use
kb-index query "API endpoints for users" --format json
```

## How It Works

KB-Index operates in two main phases:

1. **Indexing Phase**:
   - Files are read and split into chunks of approximately 10 lines each
   - Each chunk is converted to a vector embedding using OpenAI's text-embedding-3-large model
   - Embeddings are stored in ChromaDB along with metadata about the source file

2. **Query Phase**:
   - Your natural language query is converted to an embedding using the same model
   - ChromaDB performs a similarity search to find the most relevant chunks
   - Results are displayed with syntax highlighting and source information

## Supported File Types

Currently, KB-Index supports the following file types:
- Markdown (.md)
- Rust (.rs)
- TypeScript (.ts, .tsx)
- JavaScript (.js, .jsx)

## Use Cases

- **Project Onboarding**: Quickly find relevant code when joining a new project
- **Documentation Search**: Search through documentation written in Markdown
- **Code Navigation**: Find implementations of specific features across a large codebase
- **Knowledge Management**: Build a searchable knowledge base of your team's code and documentation

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [OpenAI](https://openai.com/) for their text embedding models
- [ChromaDB](https://www.trychroma.com/) for the vector database
- [Rust](https://www.rust-lang.org/) and its amazing ecosystem

---

Built with ❤️ by [Your Name/Organization]
