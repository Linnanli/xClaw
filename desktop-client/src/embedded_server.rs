use std::time::Duration;
use tracing::info;

/// Port for the external IronClaw server
pub const EMBEDDED_SERVER_PORT: u16 = 38080;

/// Check if the external IronClaw server is running
pub async fn check_server_health() -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("http://127.0.0.1:{}/api/health", EMBEDDED_SERVER_PORT);

    info!("Checking external IronClaw server at {}", url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        info!("External IronClaw server is healthy");
        Ok(())
    } else {
        Err(format!("Health check failed with status: {}", response.status()).into())
    }
}

/// Get the external server URL
pub fn get_server_url() -> String {
    format!("http://127.0.0.1:{}", EMBEDDED_SERVER_PORT)
}

/// Print instructions for starting the external IronClaw server
pub fn print_server_instructions() {
    eprintln!(
        "\n⚠️  External IronClaw server is not running on port {}",
        EMBEDDED_SERVER_PORT
    );
    eprintln!("\n📋 To start the IronClaw server:");
    eprintln!("   1. Open a new terminal");
    eprintln!("   2. Run: cargo run -- run --no-onboard");
    eprintln!("   3. Wait for the server to start");
    eprintln!("   4. Restart the Desktop Client\n");
    eprintln!("💡 The Desktop Client will continue, but chat features will not work.\n");
}
