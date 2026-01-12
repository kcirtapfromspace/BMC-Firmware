#!/usr/bin/env python3
"""Add network configuration support to bmcd."""
import sys
import os
import shutil

def main():
    if len(sys.argv) < 2:
        print("Usage: add_network_config.py <srcdir>")
        sys.exit(1)

    srcdir = sys.argv[1]
    script_dir = os.path.dirname(os.path.abspath(__file__))

    # Copy network_config.rs
    src_file = os.path.join(script_dir, "network_config.rs")
    dst_file = os.path.join(srcdir, "src", "app", "network_config.rs")
    shutil.copy(src_file, dst_file)
    print(f"Copied network_config.rs to {dst_file}")

    # Add module to app.rs
    app_rs = os.path.join(srcdir, "src", "app.rs")
    with open(app_rs, 'r') as f:
        content = f.read()
    if 'pub mod network_config;' not in content:
        content = content.replace(
            'pub mod event_application;',
            'pub mod event_application;\npub mod network_config;'
        )
        with open(app_rs, 'w') as f:
            f.write(content)
        print("Added network_config module to app.rs")

    # Modify legacy.rs
    legacy_rs = os.path.join(srcdir, "src", "api", "legacy.rs")
    with open(legacy_rs, 'r') as f:
        content = f.read()

    # Add import
    import_line = 'use crate::app::network_config::{self, BondingMode, NetworkConfigRequest};'
    if import_line not in content:
        content = content.replace(
            'use crate::app::transfer_action::InitializeTransfer;',
            f'{import_line}\nuse crate::app::transfer_action::InitializeTransfer;'
        )
        print("Added network_config import to legacy.rs")

    # Add handler routes
    handler_routes = '''        ("network_config", false) => get_network_config().await.into(),
        ("network_config", true) => set_network_config_handler(query).await.into(),
        ("about", false) => get_about().await.into(),'''

    if '("network_config", false)' not in content:
        content = content.replace(
            '        ("about", false) => get_about().await.into(),',
            handler_routes
        )
        print("Added network_config handlers to api_entry")

    # Add handler functions before #[cfg(test)]
    handler_functions = '''
async fn get_network_config() -> LegacyResult<serde_json::Value> {
    match network_config::get_network_config().await {
        Ok(config) => Ok(serde_json::to_value(config).unwrap_or_default()),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get network config: {}", e)).into())
    }
}

async fn set_network_config_handler(query: Query) -> LegacyResult<()> {
    let enabled = query.get("enabled").map(|s| s == "1" || s.to_lowercase() == "true").unwrap_or(false);
    let mode = query.get("mode").and_then(|s| BondingMode::from_str(s)).unwrap_or_default();
    let miimon = query.get("miimon").and_then(|s| s.parse().ok()).unwrap_or(100);
    let config = NetworkConfigRequest { bonding_enabled: enabled, bonding_mode: mode, miimon };
    if let Err(e) = network_config::set_network_config(config).await {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set network config: {}", e)).into());
    }
    let apply = query.get("apply").map(|s| s == "1" || s.to_lowercase() == "true").unwrap_or(false);
    if apply {
        if let Err(e) = network_config::apply_network_config().await {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to apply network config: {}", e)).into());
        }
    }
    Ok(())
}

#[cfg(test)]'''

    if 'async fn get_network_config()' not in content:
        content = content.replace('#[cfg(test)]', handler_functions)
        print("Added handler functions to legacy.rs")

    with open(legacy_rs, 'w') as f:
        f.write(content)

    print("Network config support added to bmcd")

if __name__ == "__main__":
    main()
