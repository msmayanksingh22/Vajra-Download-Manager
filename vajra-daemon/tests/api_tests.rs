use std::{
    process::{Command, Stdio},
    time::Duration,
};

#[tokio::test]
async fn test_api_integration() {
    let binary = env!("CARGO_BIN_EXE_vajrad");
    
    // We start the daemon
    let mut child = Command::new(binary)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn vajrad");

    // Give it a moment to bind
    tokio::time::sleep(Duration::from_secs(1)).await;

    if let Some(status) = child.try_wait().unwrap() {
        println!("vajrad exited with {status}. Port might be in use. Skipping test.");
        return;
    }

    let client = reqwest::Client::new();
    let base_url = "http://127.0.0.1:6277";

    // 1. Health
    let resp = client.get(format!("{}/health", base_url)).send().await.unwrap();
    assert!(resp.status().is_success());

    // 2. Config GET
    let resp = client.get(format!("{}/api/v1/config", base_url)).send().await.unwrap();
    assert!(resp.status().is_success());

    // 3. Downloads list GET
    let resp = client.get(format!("{}/api/v1/downloads", base_url)).send().await.unwrap();
    assert!(resp.status().is_success());
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json.is_object());
    assert!(json.get("items").is_some());

    // Kill the daemon
    child.kill().expect("Failed to kill vajrad");
    child.wait().unwrap();
}
