// Copyright 2024 Turing Machines
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Network configuration module for bonding support
//!
//! This module manages network bonding configuration via simple config files:
//! - /etc/bonding.enabled - marker file to enable bonding
//! - /etc/bonding.conf - contains the bonding mode (e.g., "802.3ad")
//!
//! The actual bonding setup is done by /etc/init.d/S45bonding

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tokio::fs;

const BONDING_CONF: &str = "/etc/bonding.conf";
const BONDING_CONF_OVERLAY: &str = "/mnt/overlay/upper/etc/bonding.conf";
const BONDING_ENABLED: &str = "/etc/bonding.enabled";
const BONDING_ENABLED_OVERLAY: &str = "/mnt/overlay/upper/etc/bonding.enabled";
const BONDING_PROC: &str = "/proc/net/bonding/bond0";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum BondingMode {
    BalanceRr,
    ActiveBackup,
    BalanceXor,
    Broadcast,
    #[serde(rename = "802.3ad")]
    Lacp,
    BalanceTlb,
    BalanceAlb,
}

impl BondingMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            BondingMode::BalanceRr => "balance-rr",
            BondingMode::ActiveBackup => "active-backup",
            BondingMode::BalanceXor => "balance-xor",
            BondingMode::Broadcast => "broadcast",
            BondingMode::Lacp => "802.3ad",
            BondingMode::BalanceTlb => "balance-tlb",
            BondingMode::BalanceAlb => "balance-alb",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "balance-rr" | "0" => Some(BondingMode::BalanceRr),
            "active-backup" | "1" => Some(BondingMode::ActiveBackup),
            "balance-xor" | "2" => Some(BondingMode::BalanceXor),
            "broadcast" | "3" => Some(BondingMode::Broadcast),
            "802.3ad" | "4" => Some(BondingMode::Lacp),
            "balance-tlb" | "5" => Some(BondingMode::BalanceTlb),
            "balance-alb" | "6" => Some(BondingMode::BalanceAlb),
            _ => None,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            BondingMode::BalanceRr => "Round-robin - requires switch support",
            BondingMode::ActiveBackup => "Active-backup - failover only",
            BondingMode::BalanceXor => "XOR - requires switch support",
            BondingMode::Broadcast => "Broadcast - all slaves transmit",
            BondingMode::Lacp => "802.3ad LACP - requires switch LACP support",
            BondingMode::BalanceTlb => "Transmit load balancing",
            BondingMode::BalanceAlb => "Adaptive load balancing - no switch config required",
        }
    }
}

impl Default for BondingMode {
    fn default() -> Self {
        BondingMode::ActiveBackup  // Safe default - works with single cable
    }
}

#[derive(Debug, Serialize)]
pub struct NetworkConfig {
    pub bonding_enabled: bool,
    pub bonding_mode: Option<BondingMode>,
    pub bonding_active: bool,
    pub slaves: Vec<String>,
    pub miimon: u32,
}

pub async fn is_bonding_active() -> bool {
    Path::new(BONDING_PROC).exists()
}

pub async fn get_network_config() -> anyhow::Result<NetworkConfig> {
    let bonding_active = is_bonding_active().await;
    let bonding_enabled = Path::new(BONDING_ENABLED).exists();

    // Read configured mode from bonding.conf
    let configured_mode = if let Ok(content) = fs::read_to_string(BONDING_CONF).await {
        BondingMode::from_str(content.trim())
    } else {
        None
    };

    let mut bonding_mode = configured_mode;
    let mut slaves = Vec::new();
    let mut miimon = 100;

    // If bonding is active, read actual state from /proc
    if bonding_active {
        if let Ok(bond_status) = fs::read_to_string(BONDING_PROC).await {
            for line in bond_status.lines() {
                if line.starts_with("Bonding Mode:") {
                    let mode_str = line.split(':').nth(1).unwrap_or("").trim();
                    bonding_mode = match mode_str {
                        s if s.contains("balance-rr") || s.contains("round-robin") => Some(BondingMode::BalanceRr),
                        s if s.contains("active-backup") => Some(BondingMode::ActiveBackup),
                        s if s.contains("balance-xor") || s.contains("XOR") => Some(BondingMode::BalanceXor),
                        s if s.contains("broadcast") => Some(BondingMode::Broadcast),
                        s if s.contains("802.3ad") || s.contains("LACP") => Some(BondingMode::Lacp),
                        s if s.contains("balance-tlb") || s.contains("transmit") => Some(BondingMode::BalanceTlb),
                        s if s.contains("balance-alb") || s.contains("adaptive") => Some(BondingMode::BalanceAlb),
                        _ => bonding_mode,
                    };
                } else if line.starts_with("MII Polling Interval") {
                    if let Some(val) = line.split(':').nth(1) {
                        miimon = val.trim().trim_end_matches(" ms").parse().unwrap_or(100);
                    }
                } else if line.starts_with("Slave Interface:") {
                    if let Some(iface) = line.split(':').nth(1) {
                        slaves.push(iface.trim().to_string());
                    }
                }
            }
        }
    }

    Ok(NetworkConfig { bonding_enabled, bonding_mode, bonding_active, slaves, miimon })
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfigRequest {
    pub bonding_enabled: bool,
    #[serde(default)]
    pub bonding_mode: BondingMode,
    #[serde(default = "default_miimon")]
    pub miimon: u32,
}

fn default_miimon() -> u32 { 100 }

pub async fn set_network_config(config: NetworkConfigRequest) -> anyhow::Result<()> {
    // Ensure overlay directory exists
    if let Some(parent) = Path::new(BONDING_CONF_OVERLAY).parent() {
        fs::create_dir_all(parent).await?;
    }

    if config.bonding_enabled {
        // Write bonding mode to config file
        let mode_str = format!("{}\n", config.bonding_mode.as_str());
        fs::write(BONDING_CONF_OVERLAY, &mode_str).await?;
        fs::write(BONDING_CONF, &mode_str).await?;

        // Create enabled marker file
        fs::write(BONDING_ENABLED_OVERLAY, "").await?;
        fs::write(BONDING_ENABLED, "").await?;
    } else {
        // Remove enabled marker file
        let _ = fs::remove_file(BONDING_ENABLED_OVERLAY).await;
        let _ = fs::remove_file(BONDING_ENABLED).await;
    }

    Ok(())
}

pub async fn apply_network_config() -> anyhow::Result<()> {
    tokio::task::spawn_blocking(|| {
        // Restart S45bonding to apply the new configuration
        Command::new("sh").arg("-c").arg("/etc/init.d/S45bonding restart").status()
    }).await??;
    Ok(())
}
