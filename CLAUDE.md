# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust rewrite of a Python luxury watch monitoring system that scrapes German watch dealer websites, detects new listings, and sends Discord notifications. The application runs continuously with 60-second intervals between scraping cycles.

## Commands

### Build & Run
```bash
# Development build with debug info
cargo build

# Optimized production build
cargo build --release

# Run in development mode
cargo run

# Run with logging enabled
RUST_LOG=watch_monitor=info cargo run --release
```

### Development
```bash
# Check for compilation errors without building
cargo check

# Run linter
cargo clippy

# Format code
cargo fmt

# Run tests (when implemented)
cargo test

# Run a specific test
cargo test test_name
```

## Architecture

### Core Design Principles
1. **Async Concurrency**: All scrapers run concurrently using Tokio runtime
2. **Type Safety**: NewType pattern for WatchId, Price, Reference to prevent type confusion
3. **Send Trait Compliance**: Scrapers extract data synchronously before async operations to avoid Send issues
4. **Exact Feature Parity**: Discord notifications must match Python format exactly

### Key Components

**WatchScraper Trait** (`src/scrapers/mod.rs`)
- Async trait that all site-specific scrapers implement
- Methods: `scrape()`, `site_config()`, `site_key()`
- Scrapers must handle Send trait by extracting HTML data synchronously

**Composite ID Generation** (`src/models/watch.rs`)
- Uses MD5 hashing to generate unique watch IDs
- Must match Python implementation exactly for data compatibility
- Fallback logic when essential fields are missing

**Discord Formatting** (`src/discord/embed.rs`)
- Field ordering: Price → Reference → Chrono24 → Separator → Year/Condition/Box/Papers
- Conditional fields: Only show non-❓ values
- Site-specific colors in config

**Storage Layer** (`src/storage/sqlite.rs`)
- SQLite with Arc<Mutex<Connection>> for thread safety
- Single table: `seen_watches(site, watch_id)`
- Replaces Python's JSON persistence

### Scraper Implementation Pattern

When implementing new scrapers:

1. Create struct with site-specific data extraction
2. Extract all HTML data synchronously into intermediate structs
3. Process async operations (detail page fetches) after extraction
4. Use site-specific condition mapping and parsing utilities

Example pattern from working scrapers:
```rust
// Synchronous extraction
let watch_data = extract_watch_data(&html, &base_url)?;

// Async processing
for data in watch_data {
    let listing = self.process_watch(data, client, site_config).await?;
}
```

### Critical Implementation Details

**Price Handling**
- EUR prices use regex parsing with format_price_eur_display()
- TropicalWatch requires USD→EUR conversion via exchange rate API
- Price for hash vs display price distinction is critical

**HTML Parsing**
- Use scraper 0.17 (not latest) to avoid edition2024 issues
- Parse tables with parse_table_th_td() utility
- Handle both German and English headers

**Error Handling**
- Site failures must not affect other scrapers
- Use anyhow for error context
- Log errors but continue operation

## Pending Scrapers

**TropicalWatch**: USD prices, exchange rate conversion ready in utils
**JuwelierExchange**: Standard German site pattern
**WatchOut**: Shopify with JSON-LD structured data
**Rueschenbeck**: Custom HTML structure

## Discord Webhook Requirements

Must maintain exact format:
- Emoji prefixes for all fields
- Bold values with ** markdown
- Zero-width space separator between inline groups
- Timestamp in footer with site name
- Chrono24 search URL generation