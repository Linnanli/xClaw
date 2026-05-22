//! Manager unit tests moved here from `crates/dasclaw_channels/src/manager.rs`
//! (F4.5 first slice): these depend on `crate::testing::StubChannel` which lives
//! in `desktop-client/ironclaw`, so they run as integration tests against the
//! public `dasclaw_channels::ChannelManager` API.

use dasclaw_channels::{ChannelManager, IncomingMessage, OutgoingResponse};
use futures::StreamExt;
use ironclaw::testing::StubChannel;

#[tokio::test]
async fn test_add_and_start_all() {
    let manager = ChannelManager::new();
    let (stub, sender) = StubChannel::new("test");

    manager.add(Box::new(stub)).await;

    let mut stream = manager.start_all().await.expect("start_all failed");

    sender
        .send(IncomingMessage::new("test", "user1", "hello"))
        .await
        .expect("send failed");

    let msg = stream.next().await.expect("stream ended");
    assert_eq!(msg.content, "hello");
    assert_eq!(msg.channel, "test");
}

#[tokio::test]
async fn test_respond_routes_to_correct_channel() {
    let manager = ChannelManager::new();
    let (stub, _sender) = StubChannel::new("alpha");

    let responses = stub.captured_responses_handle();
    manager.add(Box::new(stub)).await;

    let msg = IncomingMessage::new("alpha", "user1", "request");
    manager
        .respond(&msg, OutgoingResponse::text("reply"))
        .await
        .expect("respond failed");

    let captured = responses.lock().expect("poisoned");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].1.content, "reply");
}

#[tokio::test]
async fn test_respond_unknown_channel_errors() {
    let manager = ChannelManager::new();
    let msg = IncomingMessage::new("nonexistent", "user1", "test");
    let result = manager.respond(&msg, OutgoingResponse::text("hi")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_health_check_all() {
    let manager = ChannelManager::new();
    let (stub1, _) = StubChannel::new("healthy");
    let (stub2, _) = StubChannel::new("sick");
    stub2.set_healthy(false);

    manager.add(Box::new(stub1)).await;
    manager.add(Box::new(stub2)).await;

    let results = manager.health_check_all().await;
    assert!(results["healthy"].is_ok());
    assert!(results["sick"].is_err());
}

#[tokio::test]
async fn test_start_all_no_channels_errors() {
    let manager = ChannelManager::new();
    let result = manager.start_all().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_injection_channel_merges() {
    let manager = ChannelManager::new();
    let (stub, _sender) = StubChannel::new("real");
    manager.add(Box::new(stub)).await;

    let mut stream = manager.start_all().await.expect("start_all failed");

    let inject_tx = manager.inject_sender();
    inject_tx
        .send(IncomingMessage::new(
            "injected",
            "system",
            "background alert",
        ))
        .await
        .expect("inject failed");

    let msg = stream.next().await.expect("stream ended");
    assert_eq!(msg.content, "background alert");
}

#[tokio::test]
async fn test_hot_add_replaces_existing_channel() {
    // Regression: hot_add must shut down the existing channel before replacing it,
    // to prevent duplicate SSE consumers from running in parallel.
    let manager = ChannelManager::new();
    let (stub1, _tx1) = StubChannel::new("relay");
    manager.add(Box::new(stub1)).await;
    let mut stream = manager.start_all().await.expect("start_all");

    let (stub2, tx2) = StubChannel::new("relay");
    manager.hot_add(Box::new(stub2)).await.expect("hot_add");

    tx2.send(IncomingMessage::new("relay", "u1", "from new"))
        .await
        .expect("send");
    let msg = stream.next().await.expect("stream");
    assert_eq!(msg.content, "from new");

    // Verify only one channel entry remains (private field replaced by public
    // health_check_all view which iterates the same map).
    let health = manager.health_check_all().await;
    assert_eq!(health.len(), 1);
    assert!(health.contains_key("relay"));
}
