use std::path::Path;

use base64::Engine;
use sha2::{Digest, Sha256};

#[test]
fn test_extension_id_matches_public_key() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("vajra-extension/public/manifest.json");
    assert!(manifest_path.exists(), "manifest.json must exist");

    let content = std::fs::read_to_string(&manifest_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    let key_base64 = json["key"]
        .as_str()
        .expect("key field required in manifest.json");

    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(key_base64)
        .expect("Valid base64 public key");
    let hash = Sha256::digest(&key_bytes);
    let hex_32 = hex::encode(&hash[..16]);
    let calculated_id: String = hex_32
        .chars()
        .map(|c| {
            let val = c.to_digit(16).unwrap() as u8;
            (b'a' + val) as char
        })
        .collect();

    assert_eq!(
        calculated_id, "mfdepghakanbpamaakojoaogglepehfh",
        "Calculated extension ID must match expected ID"
    );
}

#[test]
fn test_extension_send_native_message_name() {
    let bg_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("vajra-extension/src/background.ts");
    assert!(bg_path.exists(), "background.ts must exist");

    let content = std::fs::read_to_string(&bg_path).unwrap();

    // Verify all native messaging calls use "com.vajra.manager"
    assert!(
        content.contains("sendNativeMessage('com.vajra.manager'"),
        "background.ts must call sendNativeMessage with com.vajra.manager"
    );
    assert!(
        !content.contains("com.vajra.downloadmanager"),
        "background.ts must not contain legacy com.vajra.downloadmanager"
    );
}

#[test]
fn test_nsis_installer_native_host_registration_consistency() {
    let nsis_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("vajra-ui-tauri/src-tauri/assets/nsis-hooks.nsh");
    assert!(nsis_path.exists(), "nsis-hooks.nsh must exist");

    let content = std::fs::read_to_string(&nsis_path).unwrap();

    // 1. Host identity in manifest
    assert!(
        content.contains(r#""name": "com.vajra.manager""#),
        "NSIS manifest name must be com.vajra.manager"
    );

    // 2. Manifest path must point directly to protocol-speaking vajra-cli.exe
    assert!(
        content.contains(r#""path": "$2\\\\vajra-cli.exe""#),
        "NSIS manifest path must point directly to vajra-cli.exe"
    );

    // 3. Allowed origin must match extension ID
    assert!(
        content.contains("chrome-extension://mfdepghakanbpamaakojoaogglepehfh/"),
        "NSIS allowed_origins must match calculated extension ID"
    );

    // 4. Registry keys for Chrome and Edge
    assert!(
        content.contains(r#"Software\Google\Chrome\NativeMessagingHosts\com.vajra.manager"#),
        "NSIS must register Chrome NativeMessagingHost com.vajra.manager"
    );
    assert!(
        content.contains(r#"Software\Microsoft\Edge\NativeMessagingHosts\com.vajra.manager"#),
        "NSIS must register Edge NativeMessagingHost com.vajra.manager"
    );

    // 5. Cleanup of legacy com.vajra.downloadmanager
    assert!(
        content.contains("com.vajra.downloadmanager"),
        "NSIS must contain cleanup code for legacy com.vajra.downloadmanager"
    );
}

#[test]
fn test_install_ps1_registration_consistency() {
    let ps1_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("installer/install.ps1");
    assert!(ps1_path.exists(), "install.ps1 must exist");

    let content = std::fs::read_to_string(&ps1_path).unwrap();

    // 1. vajra-cli.exe included in installed files
    assert!(
        content.contains("'vajra-cli.exe'"),
        "install.ps1 must include vajra-cli.exe in RequiredFiles"
    );

    // 2. Manifest name
    assert!(
        content.contains(r#""name": "com.vajra.manager""#),
        "install.ps1 manifest name must be com.vajra.manager"
    );

    // 3. Manifest path
    assert!(
        content.contains(r#""path": "$escapedPath""#),
        "install.ps1 manifest path must be set"
    );

    // 4. Allowed origins
    assert!(
        content.contains("chrome-extension://mfdepghakanbpamaakojoaogglepehfh/"),
        "install.ps1 allowed_origins must match extension ID"
    );
}
