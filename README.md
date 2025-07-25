# Watch Monitor - Rust Edition 🦀⌚

A high-performance luxury watch monitoring system written in Rust. This application continuously monitors German watch dealer websites for new listings and sends Discord webhook notifications.

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Discord](https://img.shields.io/badge/Discord-%235865F2.svg?style=for-the-badge&logo=discord&logoColor=white)

## 🚀 Performance

This Rust implementation offers significant improvements over the original Python version:
- **6x faster**: Parallel scraping reduces cycle time from ~6 minutes to <30 seconds
- **Memory efficient**: Zero-copy parsing and optimized string handling
- **Concurrent**: All sites are scraped in parallel using Tokio
- **Type safe**: Compile-time guarantees prevent runtime errors

## Features

- **Concurrent Scraping**: Uses Tokio async runtime to scrape all sites in parallel
- **Exact Feature Parity**: Maintains identical Discord notification formatting as the Python version
- **High Performance**: Leverages Rust's zero-cost abstractions and memory safety
- **SQLite Storage**: Efficient persistent storage for tracking seen watches
- **Robust Error Handling**: Graceful degradation with retry logic
- **Type Safety**: Strongly-typed domain models with NewType pattern

## Supported Sites

- ✅ World of Time (worldoftime.de)
- ✅ Grimmeissen (grimmeissen.de)
- ✅ Tropical Watch (tropicalwatch.com) - with USD to EUR conversion
- ✅ Juwelier Exchange (juwelier-exchange.de)
- ✅ Watch Out (watch-out.shop) - with Shopify integration
- ✅ Rüschenbeck (rueschenbeck.de)

## Building

```bash
cargo build --release
```

## Running

### Development Mode
```bash
# Run with debug info and logging
RUST_LOG=watch_monitor=info cargo run
```

### Production Mode
```bash
# Run optimized build
cargo run --release

# Run with specific log level
RUST_LOG=watch_monitor=warn cargo run --release
```

The application will:
1. Initialize SQLite database (`watch_monitor.db`) for persistence
2. Start scraping all 6 sites concurrently
3. Check every 60 seconds for new listings
4. Send Discord notifications for new watches
5. Track seen watches to avoid duplicate notifications

### First Run Notes
- The first run will create the SQLite database
- All current watches will be marked as "seen" (no notifications)
- Only new watches added after the first run will trigger notifications
- To test notifications, you can delete `watch_monitor.db` and run again

## Configuration

Currently, configuration is hardcoded in `src/config.rs`. To use this monitor:

1. Edit `src/config.rs` and add your Discord webhook URLs:
```rust
SiteConfig {
    name: "World of Time".to_string(),
    base_url: "https://worldoftime.de".to_string(),
    url: "https://worldoftime.de/luxury-watches".to_string(),
    webhook: "YOUR_DISCORD_WEBHOOK_URL_HERE".to_string(),
    color: 0x2F4F4F, // Dark Slate Gray
}
```

2. Recompile and run the application

Future versions will support external configuration files.

## Discord Notifications

Each notification includes:
- Watch brand, model, and reference
- Price in EUR
- Year of manufacture
- Condition rating
- Box/papers status
- Case material and diameter
- Direct link to listing
- Chrono24 search link
- Thumbnail image

## Development

### Project Structure

```
src/
├── main.rs           # Async runtime and main loop
├── config.rs         # Configuration structures
├── models/           # Domain models
├── scrapers/         # Site-specific scrapers
├── parsers/          # Common parsing utilities
├── discord/          # Discord webhook integration
├── storage/          # SQLite persistence
└── utils/            # HTTP client and utilities
```

### Adding a New Scraper

1. Create a new file in `src/scrapers/`
2. Implement the `WatchScraper` trait
3. Add the scraper to the main loop in `main.rs`

## 🔧 Technical Details

### Architecture
- **Async/Await**: Built on Tokio for efficient I/O operations
- **Type Safety**: NewType pattern for domain modeling (WatchId, Price, Reference)
- **Error Handling**: Comprehensive error handling with `anyhow` and `Result` types
- **Storage**: SQLite with thread-safe access patterns

### Dependencies
- `tokio` - Async runtime
- `reqwest` - HTTP client with connection pooling
- `scraper` - HTML parsing (similar to Python's BeautifulSoup)
- `rusqlite` - SQLite database integration
- `serde` - Serialization/deserialization
- `tracing` - Structured logging

## 📊 Monitoring Dashboard

Each Discord notification includes:
- 🏷️ Brand, model, and reference number
- 💰 Price in EUR (with USD conversion for TropicalWatch)
- 📅 Year of manufacture
- ⭐ Condition rating
- 📦 Box and 📄 Papers status
- 🔗 Direct link to listing
- 🔍 Chrono24 search link for price comparison

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Original Python implementation that inspired this Rust version
- The Rust community for excellent async libraries
- Watch enthusiast communities for their passion

---

**Note**: This tool is for personal use. Please respect the websites' terms of service and implement appropriate rate limiting.