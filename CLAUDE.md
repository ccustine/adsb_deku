# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

adsb_deku is a Rust library for decoding ADS-B (Automatic Dependent Surveillance-Broadcast) messages from aircraft. The project consists of multiple workspace crates:

- `libadsb_deku/` - Core library for ADS-B protocol decoding using the deku crate
- `apps/` - Client applications (`radar` TUI and `1090` decoder)  
- `rsadsb_common/` - Common data structures and airplane tracking logic
- `ensure_no_std/` - Validates no_std compatibility

## Architecture

### Core Library (libadsb_deku)
- Main entry point: `Frame::from_bytes()` for decoding raw ADS-B bytes
- Protocol support includes DF (Downlink Format) 0,4,5,11,16,17,18,19,20,21,24-31
- Extensive ADS-B message type support via `ME` (Message Element) enum
- Uses deku crate for binary deserialization with custom readers
- Supports both `std` and `no_std` environments with `alloc` feature

### Common Library (rsadsb_common)
- `Airplanes` struct tracks aircraft state using `BTreeMap<ICAO, AirplaneState>`
- CPR (Compact Position Reporting) decoding for aircraft positions
- Distance calculations using Haversine formula
- Aircraft filtering by time and range

### Applications
- `radar` - Terminal UI using ratatui with multiple tabs (Map, Coverage, Aircraft, Stats)
- `1090` - Command-line decoder similar to dump1090-fa output format
- Both connect to ADS-B demodulation servers (like dump1090_rs) via TCP
- Support both raw hex format (port 30002) and BEAST mode binary format (port 30005)

## Common Development Commands

### Building
```bash
cargo build                    # Build all workspace members
cargo build --release         # Release build
cargo build --bin radar       # Build specific binary
cargo build --bin 1090        # Build decoder app
```

### Testing
```bash
cargo test                     # Run all tests
cargo test --workspace        # Test entire workspace
cargo fuzz run fuzz_target_1  # Run fuzzing (in libadsb_deku/)
```

### Code Quality
```bash
cargo +nightly fmt            # Format code (requires nightly)
cargo clippy --workspace -- -D warnings  # Lint with warnings as errors
```

### Running Applications
```bash
# Radar TUI (requires lat/long of antenna)
cargo run --bin radar --release -- --lat="50.0" --long="50.0"

# Radar TUI with BEAST mode (connects to port 30005 by default)
cargo run --bin radar --release -- --lat="50.0" --long="50.0" --beast-mode

# 1090 decoder with debug output
cargo run --bin 1090 --release -- --debug

# 1090 decoder with BEAST mode
cargo run --bin 1090 --release -- --beast-mode --debug
```

### Benchmarking
```bash
cargo bench                    # Run decoding benchmarks (in libadsb_deku/)
```

## Key Development Notes

### Minimum Rust Version
Project requires Rust 1.84.0 or later (defined in workspace Cargo.toml).

### Features
- Default features enable `std` and `alloc`
- `no_std` support available with `default-features = false, features = ["alloc"]`
- Optional `serde` support for serialization

### Message Flow
1. Raw ADS-B bytes → `Frame::from_bytes()` → parsed `DF` variants
2. `DF::ADSB` messages → `rsadsb_common::Airplanes::action()` → tracked aircraft state
3. Applications display/process the tracked aircraft data

### Testing Strategy
- Unit tests for protocol decoding
- Integration tests with real ADS-B message files (lax-messages.txt)
- Fuzz testing for protocol robustness
- Applications can run with `--panic-decode` and `--panic-display` for testing

### Protocol Implementation
The library implements ICAO standards for ADS-B with extensive coverage of:
- Downlink Format types (surveillance, extended squitter, etc.)
- Message Elements for position, velocity, identification
- CPR position decoding algorithms
- CRC validation and ICAO address extraction

### BEAST Mode Support
Applications support BEAST mode binary format in addition to raw hex format:
- BEAST mode provides additional metadata (timestamps, signal strength)
- Frame format: `<esc> <type> <6-byte timestamp> <signal> <message-data>`
- Escape character: 0x1A with escape sequence handling
- Automatic port selection: 30005 for BEAST mode, 30002 for raw mode
- Compatible with dump1090-fa BEAST output and Mode S Beast hardware