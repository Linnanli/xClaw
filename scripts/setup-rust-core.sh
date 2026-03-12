#!/bin/bash
# scripts/setup-rust-core.sh
# Installs core engineering crates for IronClaw

echo "Adding core engineering crates..."

# Error Handling
cargo add thiserror
cargo add anyhow

# Async Runtime
cargo add tokio --features full

# Logging & Tracing
cargo add tracing
cargo add tracing-subscriber

# Serialization
cargo add serde --features derive
cargo add serde_json

# Testing Utilities
cargo add proptest --dev
cargo install cargo-nextest --locked

echo "Core engineering crates installed successfully."
