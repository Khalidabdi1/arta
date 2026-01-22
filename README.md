# Arta

> Query your system with SQL-like commands

Arta is a command-line tool that lets you query and manage system state using a familiar SQL-like syntax. Think of it as SQL for your operating system.

<p align="center">
  <img 
    src="https://pub-990fcdb7ef2f426e8a1d0578653b21c4.r2.dev/ChatGPT%20Image%2022%20%D9%8A%D9%86%D8%A7%D9%8A%D8%B1%202026%D8%8C%2005_34_12%20%D9%85.png"
    width="720"
    style="border-radius: 18px;"
  />
</p>

## Features

- **SQL-like syntax** - Familiar query language for system operations
- **Script files** - Write and execute `.arta` scripts
- **LIFE monitoring** - Real-time reactive monitoring of system resources
- **Containers** - Isolated execution environments with their own context
- **Control flow** - FOR loops, IF statements, and variables
- **Read-only by default** - Safe exploration without accidental modifications
- **Actions with safeguards** - Destructive operations require explicit flags
- **Multiple output formats** - Human-readable or JSON for scripting
- **Cross-platform** - Works on macOS, Linux, and Windows
- **Interactive REPL** - Explore your system interactively

## Installation

### From Release Binaries

Download the latest release for your platform from the [Releases](https://github.com/yourusername/arta/releases) page.

```bash
# Linux/macOS
tar xzf arta-linux-x86_64.tar.gz
chmod +x arta
sudo mv arta /usr/local/bin/

# Or add to your PATH
export PATH="$PATH:/path/to/arta"
```

### From Source

```bash
# Clone the repository
git clone https://github.com/khalidabdi1/arta.git
cd arta

# Build
cargo build --release

# Install (optional)
cargo install --path .
```

### With Cargo

```bash
cargo install arta
```

## Quick Start

```bash
# Query CPU information
arta query "SELECT CPU *"

# Query memory usage
arta query "SELECT MEMORY *"

# Query processes with high CPU usage
arta query "SELECT PROCESS * WHERE cpu > 10"

# Output as JSON
arta --json query "SELECT SYSTEM *"

# Run a script
arta run examples/health_check.arta

# Live monitoring
arta life battery

# Explain a script without executing
arta explain examples/cleanup.arta

# Start interactive REPL (requires --features repl)
arta repl
```

## CLI Commands

```
arta [OPTIONS] <COMMAND>

Commands:
  query       Execute a single query
  run         Run an Arta script file (.arta)
  life        Start live monitoring mode
  explain     Explain a script or query without executing
  repl        Start interactive REPL mode
  containers  List all containers

Options:
  --dry-run         Show what would happen without executing
  --allow-actions   Enable destructive actions (DELETE, KILL)
  --json            Output in JSON format
  --container       Run in a specific container
  -v, --verbose     Verbose output
  -h, --help        Print help
  -V, --version     Print version
```

## Query Examples

### System Information

```sql
-- CPU information
SELECT CPU *
SELECT CPU cores, usage

-- Memory usage
SELECT MEMORY *
SELECT MEMORY total, used, free

-- Disk information
SELECT DISK * FROM /

-- Network interfaces
SELECT NETWORK *

-- System details
SELECT SYSTEM *

-- Battery status (laptops)
SELECT BATTERY *
```

### Process Queries

```sql
-- All processes
SELECT PROCESS *

-- Filter by CPU usage
SELECT PROCESS * WHERE cpu > 10

-- Filter by name
SELECT PROCESS * WHERE name = "node"

-- Filter by memory (supports size units)
SELECT PROCESS * WHERE memory > 100MB
```

### File Queries

```sql
-- List files in a directory
SELECT FILES * FROM /tmp

-- Filter by extension
SELECT FILES * FROM /home WHERE extension = "log"

-- Filter by size
SELECT FILES * FROM /var/log WHERE size > 10MB

-- Read file content
SELECT CONTENT * FROM /etc/hosts
```

### Context Navigation

```sql
-- Enter a folder context
ENTER FOLDER /tmp

-- List files in current context
SELECT FILES *

-- Enter a file for inspection
ENTER FILE config.json

-- View file content
SELECT CONTENT *

-- Exit current context
EXIT

-- Show current context
SHOW CONTEXT
```

### Variables

```sql
-- Define variables
LET my_path = /tmp
LET threshold = 80
LET max_size = 100MB

-- Use in queries
SELECT FILES * FROM my_path WHERE size > max_size

-- Show all variables
SHOW VARIABLES
```

### Control Flow

```sql
-- FOR loop
FOR file IN SELECT FILES * FROM /tmp WHERE extension = "log" DO
    PRINT "Processing:", file.name;
    SELECT CONTENT * FROM file;
END FOR;

-- IF statement
IF SELECT MEMORY usage > 80 THEN
    SELECT PROCESS * WHERE memory > 100MB;
END IF;

-- IF with ELSE
IF SELECT CPU usage > 90 THEN
    PRINT "High CPU usage!";
    SELECT PROCESS * WHERE cpu > 10;
ELSE
    PRINT "CPU usage is normal";
END IF;

-- Nested control flow
FOR file IN SELECT FILES * FROM /tmp DO
    IF SELECT DISK usage > 90 THEN
        PRINT "Warning: Low disk space while processing", file.name;
    END IF;
END FOR;
```

### LIFE Monitoring

```sql
-- Monitor battery and react to changes
LIFE MONITOR BATTERY DO
    PRINT BATTERY level, BATTERY state;
    
    IF SELECT BATTERY level < 20 THEN
        PRINT "WARNING: Low battery!";
    END IF;
END LIFE;

-- Monitor CPU usage
LIFE MONITOR CPU DO
    PRINT CPU usage;
END LIFE;
```

### Containers

Containers provide isolated execution environments with their own context, variables, and options.

```sql
-- Create a basic container
CREATE CONTAINER "sandbox" DO
    LET threshold = 50;
    SELECT CPU *;
END CONTAINER;

-- Create with options
CREATE CONTAINER "safe_env" WITH READONLY DO
    -- This container cannot modify anything
    SELECT PROCESS *;
END CONTAINER;

-- Create with action permissions (requires --allow-actions flag)
CREATE CONTAINER "cleanup_env" WITH ALLOW ACTIONS DO
    -- This container can perform destructive actions
    SELECT FILES * FROM /tmp;
END CONTAINER;

-- Create with multiple options
CREATE CONTAINER "mixed" WITH ALLOW ACTIONS, READONLY DO
    SELECT SYSTEM *;
END CONTAINER;

-- Switch between containers
SWITCH CONTAINER "sandbox";

-- List all containers
LIST CONTAINERS;

-- Destroy a container
DESTROY CONTAINER "sandbox";

-- Export container state to a file
EXPORT CONTAINER "my_env" TO /tmp/my_env.arta;
```

#### Container Options

- **READONLY** - Container cannot execute destructive actions
- **ALLOW ACTIONS** - Container can execute actions (DELETE, KILL) if `--allow-actions` flag is set

#### Running with Containers

```bash
# Run a script in a specific container
arta --container myenv run script.arta

# Start REPL in a container
arta --container dev repl

# List all containers
arta containers
```

### PRINT Command

```sql
-- Print strings and values
PRINT "Hello, World!";

-- Print system values
PRINT BATTERY level, BATTERY state;

-- Print variables
LET name = "test";
PRINT "Name is:", name;

-- Multiple expressions
PRINT CPU usage, MEMORY usage, DISK usage;
```

### Actions (Require `--allow-actions`)

```sql
-- Delete files (with safeguards)
DELETE FILES FROM /tmp WHERE size > 100MB

-- Kill processes
KILL PROCESS WHERE name = "node"
```

### Explain Mode

```sql
-- See what a command would do without executing
EXPLAIN DELETE FILES FROM /tmp WHERE extension = "log"
```

## Script Files

Arta supports script files with the `.arta` extension. Scripts can contain multiple statements, comments, variables, and control flow.

### Example Script

```sql
-- health_check.arta
-- Check system health

LET cpu_threshold = 80;
LET memory_threshold = 80;

PRINT "=== System Health Check ===";

IF SELECT CPU usage > cpu_threshold THEN
    PRINT "WARNING: High CPU usage!";
    SELECT PROCESS * WHERE cpu > 10;
END IF;

IF SELECT MEMORY usage > memory_threshold THEN
    PRINT "WARNING: High memory usage!";
    SELECT PROCESS * WHERE memory > 100MB;
END IF;

PRINT "=== Health Check Complete ===";
```

### Running Scripts

```bash
# Run a script
arta run health_check.arta

# Run with arguments
arta run cleanup.arta --arg path=/tmp --arg threshold=80

# Dry run (preview)
arta --dry-run run cleanup.arta

# With JSON output
arta --json run health_check.arta

# Enable actions
arta --allow-actions run cleanup.arta

# Explain what a script does
arta explain health_check.arta
```

### Script Validation

Scripts are validated before execution:
- Actions require `--allow-actions` flag
- LIFE blocks cannot contain destructive actions
- Warnings for dangerous patterns (e.g., DELETE without WHERE)

## Live Monitoring

The `life` command provides real-time monitoring of system resources.

```bash
# Monitor battery
arta life battery

# Monitor CPU (with 2-second interval)
arta life cpu --interval 2

# Monitor memory with JSON output
arta --json life memory

# Available targets: battery, cpu, memory, disk, network, processes
```

## Safety Features

Arta is designed with safety as a priority:

1. **Read-only by default** - All queries are non-destructive
2. **Actions require explicit flag** - Use `--allow-actions` to enable modifications
3. **Script validation** - Scripts are validated before execution
4. **Dry-run support** - Use `--dry-run` to preview changes
5. **WHERE clause warnings** - DELETE without filtering shows warnings
6. **Safety limits** - Maximum items per operation prevents accidents
7. **Protected processes** - System-critical processes cannot be killed
8. **LIFE restrictions** - Monitoring blocks can't execute destructive actions

## Comments

Arta supports multiple comment styles:

```sql
-- SQL-style comment
# Shell-style comment
// C-style comment

/* 
   Multi-line
   block comment
*/
```

## Architecture

```
arta/
├── grammar/
│   └── arta.pest        # PEG grammar definition
├── examples/            # Example .arta scripts
├── .github/workflows/   # CI/CD pipelines
└── src/
    ├── main.rs          # CLI entry point
    ├── lib.rs           # Library root
    ├── error.rs         # Error types
    ├── parser/          # Grammar parsing
    │   ├── ast.rs       # Abstract syntax tree
    │   └── grammar.rs   # Pest parser
    ├── engine/          # Execution engine
    │   ├── executor.rs
    │   ├── queries/     # Query implementations
    │   └── actions/     # Action implementations
    ├── security/        # Security & validation
    ├── output/          # Output formatting
    ├── cli/             # CLI argument parsing
    ├── context/         # Context & state management
    ├── container/       # Container management
    ├── script/          # Script runner & validation
    ├── life/            # LIFE monitoring
    └── repl/            # Interactive REPL
```

## Building

### Standard Build

```bash
cargo build --release
```

### With REPL Support

```bash
cargo build --release --features repl
```

### Run Tests

```bash
cargo test
```

## GitHub Actions

This project includes CI/CD workflows:

- **CI** (`ci.yml`): Runs on every push/PR
  - Tests on Linux, macOS, Windows
  - Linting with clippy and rustfmt
  - Code coverage reporting

- **Release** (`release.yml`): Runs on version tags
  - Builds binaries for all platforms
  - Creates GitHub releases with artifacts
  - Publishes to crates.io

To create a release:
```bash
git tag v0.4.0
git push --tags
```

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

### Development Setup

```bash
git clone https://github.com/yourusername/arta.git
cd arta
cargo build
cargo test
```

### Code Style

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy
```

## License

Dual-licensed under MIT or Apache-2.0.
