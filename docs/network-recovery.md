# TuringPi BMC Network Recovery Guide

This document provides comprehensive procedures for recovering network access to the TuringPi BMC when normal network connectivity is lost.

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [USB Console Access](#usb-console-access)
3. [Fallback IP Access](#fallback-ip-access)
4. [Network Recovery Tool](#network-recovery-tool)
5. [Common Failure Scenarios](#common-failure-scenarios)
6. [Bonding Configuration](#bonding-configuration)
7. [Switch Requirements for LACP](#switch-requirements-for-lacp)
8. [Advanced Troubleshooting](#advanced-troubleshooting)

---

## Quick Reference

| Access Method | Connection | Address |
|---------------|------------|---------|
| Fallback IP | Direct Ethernet | `192.168.0.2` |
| USB Console | Serial over USB | `/dev/ttyUSB0` at 115200 baud |
| DHCP | Network | Check router/DHCP server |

**Emergency Recovery Commands:**
```bash
# Via USB console - run interactive recovery tool
network-recovery

# Manual factory reset
/etc/init.d/S46netwatch revert

# Check current IP addresses
ip addr show br0
```

---

## USB Console Access

When network access is completely unavailable, connect via USB serial console.

### Prerequisites

- USB cable connected between TuringPi board and a host computer
- Serial terminal software (screen, minicom, or similar)

### Connection Steps

1. **From an intermediate host** (e.g., a connected node or external computer):
   ```bash
   # SSH to the host that has USB access to the BMC
   ssh think-nvidia@think-nvidia

   # Connect to the BMC serial console
   screen /dev/ttyUSB0 115200
   ```

2. **Direct USB connection** (if BMC is connected directly to your computer):
   ```bash
   # Linux/macOS
   screen /dev/ttyUSB0 115200

   # Or using minicom
   minicom -D /dev/ttyUSB0 -b 115200
   ```

3. **Log in** with root credentials (default: no password, or check your configuration)

4. **To exit screen**: Press `Ctrl+A` then `K`, then confirm with `y`

### Troubleshooting USB Console

- If `/dev/ttyUSB0` doesn't exist, check `dmesg | tail` for USB device detection
- Try `/dev/ttyUSB1` or `/dev/ttyACM0` if USB0 isn't available
- Ensure you have permissions: `sudo usermod -a -G dialout $USER` (logout/login required)

---

## Fallback IP Access

The BMC is configured with a static fallback IP that is **always accessible** regardless of DHCP or bonding state.

### Fallback IP Details

- **IP Address**: `192.168.0.2`
- **Netmask**: `255.255.255.0`
- **Interface**: `br0:1` (alias on the main bridge)

### Connecting via Fallback IP

1. **Configure your computer** with a static IP in the same subnet:
   ```bash
   # Example: Set your computer to 192.168.0.10
   sudo ip addr add 192.168.0.10/24 dev eth0
   ```

2. **Connect directly** to the BMC:
   - **SSH**: `ssh root@192.168.0.2`
   - **Web UI**: Open `http://192.168.0.2` in a browser

3. **Verify connectivity**:
   ```bash
   ping 192.168.0.2
   ```

### When Fallback IP Doesn't Work

If the fallback IP is unreachable:

1. Ensure you're on the same physical network segment (direct connection or same switch)
2. Check that your subnet doesn't conflict (no other devices at 192.168.0.2)
3. Try connecting directly with a crossover cable or via a simple unmanaged switch
4. Use USB console access as a last resort

---

## Network Recovery Tool

The BMC includes an interactive recovery tool designed for USB console use.

### Starting the Tool

```bash
network-recovery
```

### Menu Options

#### Option 1: Show Current Status

Displays comprehensive network diagnostics:
- All network interfaces with IP addresses and status
- Bridge configuration and port membership
- Bonding status (if enabled)
- Current bonding mode configuration

#### Option 2: Reset to Factory Defaults

Performs a complete network reset:
- Restores `/etc/network/interfaces` to factory default
- Removes `/etc/bonding.enabled` (disables bonding)
- Restarts networking

**Use when**: Configuration is corrupted or unknown state

#### Option 3: Disable Bonding

Disables link aggregation without full reset:
- Removes `/etc/bonding.enabled` marker file
- Stops bonding service
- Restarts networking with ge0/ge1 as direct bridge ports

**Use when**: Bonding was enabled but is causing issues

#### Option 4: Set Static IP

Configures a static IP address:
- Prompts for IP address, netmask, and gateway
- Updates `/etc/network/interfaces`
- Restarts networking

**Use when**: DHCP is unavailable or you need a fixed address

#### Option 5: Test Connectivity

Tests network connectivity:
- Pings the default gateway
- Pings 8.8.8.8 (Google DNS) to verify internet access

**Use when**: Verifying network changes were successful

---

## Common Failure Scenarios

### Scenario 1: Board Doesn't Respond After Bonding Change

**Symptoms**: Web UI and SSH become unreachable after enabling bonding

**Cause**: Switch doesn't support the configured bonding mode (especially 802.3ad/LACP)

**Solution**:
1. Wait 60 seconds - the watchdog may automatically revert
2. If no auto-revert, connect via USB console
3. Run `network-recovery` and select option 3 (Disable Bonding)

### Scenario 2: No DHCP Address Assigned

**Symptoms**: BMC boots but doesn't get an IP address

**Solution**:
1. Connect via fallback IP (192.168.0.2) or USB console
2. Run `network-recovery` and select option 4 (Set Static IP)
3. Or fix DHCP server and run `dhclient br0`

### Scenario 3: Bonding Works But Slow/Unstable

**Symptoms**: Network works but with packet loss or low throughput

**Cause**: Usually MAC address conflicts or switch misconfiguration

**Solution**:
1. Via SSH or console: `cat /proc/net/bonding/bond0`
2. Check that both slaves show "MII Status: up"
3. Verify unique MAC addresses on ge0 and ge1
4. Try a simpler bonding mode (active-backup instead of 802.3ad)

### Scenario 4: Bridge Has No Ports

**Symptoms**: `bridge link show` shows no ports on br0

**Cause**: S00dsa didn't run or DSA driver issue

**Solution**:
1. Check boot logs: `dmesg | grep -i dsa`
2. Verify DSA ports exist: `ls /sys/class/net/ | grep -E "node|ge"`
3. Manual recovery: `ip link set ge0 master br0 && ip link set ge1 master br0`

### Scenario 5: Configuration Corrupted

**Symptoms**: Network services fail to start, error messages in logs

**Solution**:
1. Connect via USB console
2. Run `network-recovery` and select option 2 (Reset to Factory Defaults)
3. Reconfigure as needed

---

## Bonding Configuration

### Overview

Link aggregation (bonding) combines ge0 and ge1 into a single logical interface for:
- **Failover**: Automatic switchover if one link fails
- **Increased bandwidth**: Up to 2 Gbps aggregate (with proper switch support)

### Configuration Files

| File | Purpose |
|------|---------|
| `/etc/bonding.enabled` | Marker file - bonding active when present |
| `/etc/bonding.conf` | Contains bonding mode (one line) |
| `/etc/network/interfaces` | Network configuration (auto-updated) |
| `/etc/network/interfaces.backup` | Backup before changes |

### Enabling Bonding

**Via Web UI** (recommended):
1. Navigate to System > Network Settings
2. Enable "Link Aggregation"
3. Select bonding mode
4. Click Apply

**Via command line**:
```bash
# Set bonding mode
echo "active-backup" > /etc/bonding.conf

# Enable bonding
touch /etc/bonding.enabled

# Restart bonding service
/etc/init.d/S45bonding restart
```

### Disabling Bonding

**Via Web UI**:
1. Navigate to System > Network Settings
2. Disable "Link Aggregation"
3. Click Apply

**Via command line**:
```bash
rm -f /etc/bonding.enabled
/etc/init.d/S45bonding stop
/etc/init.d/S40network restart
```

### Bonding Modes

| Mode | Name | Description | Switch Config Required |
|------|------|-------------|----------------------|
| `active-backup` | Failover | One active link, automatic failover | None |
| `balance-alb` | Adaptive LB | Load balancing without switch support | None |
| `802.3ad` | LACP | IEEE standard, full 2 Gbps | **Yes - LACP required** |
| `balance-rr` | Round-Robin | Packet-level load balancing | Yes |
| `balance-xor` | XOR | Hash-based distribution | Yes |
| `balance-tlb` | Transmit LB | Outbound load balancing | None |

### Best Practices

1. **Start with active-backup**: Works with any switch, provides failover
2. **Test before committing**: Use the 60-second auto-revert safety window
3. **Match switch config**: For 802.3ad, configure LACP on switch first
4. **Check logs**: `grep bonding /var/log/messages`

---

## Switch Requirements for LACP

802.3ad (LACP) mode requires proper switch configuration.

### Managed Switch Setup

1. **Create a LAG/Port Channel** on your switch for the two ports connected to TuringPi ge0 and ge1

2. **Configure LACP mode** (not static/manual LAG):
   - Cisco: `channel-group 1 mode active`
   - TP-Link: LAG > LACP > Enable
   - Ubiquiti: Create aggregate with LACP

3. **Verify LACP negotiation**:
   - Check switch shows both ports as "bundled" or "aggregated"
   - On BMC: `cat /proc/net/bonding/bond0` should show both slaves up

### Unmanaged Switch Behavior

- **Will not work with 802.3ad** - LACP requires switch participation
- **Use active-backup or balance-alb** instead for unmanaged switches
- **balance-alb** can provide some load balancing without switch support

### Common LACP Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| Only one link active | LACP not enabled on switch | Configure switch LAG |
| Links flapping | Timeout mismatch | Set LACP fast rate on switch |
| No aggregation | Different VLANs on ports | Ensure same VLAN config |
| Low throughput | Traffic not distributed | Check hash algorithm |

---

## Advanced Troubleshooting

### Viewing Logs

```bash
# Recent network-related logs
grep -E "S4[056]|bonding|network" /var/log/messages | tail -50

# Boot sequence logs
dmesg | grep -iE "dsa|eth|bond|br0"
```

### Manual Bridge Management

```bash
# Show bridge ports
bridge link show

# Add port to bridge
ip link set ge0 master br0

# Remove port from bridge
ip link set ge0 nomaster
```

### Manual Bonding Management

```bash
# Check bonding status
cat /proc/net/bonding/bond0

# View bonding module parameters
cat /sys/class/net/bond0/bonding/mode
cat /sys/class/net/bond0/bonding/slaves

# Manually create bond (for testing)
ip link add bond0 type bond mode active-backup miimon 100
ip link set ge0 master bond0
ip link set ge1 master bond0
ip link set bond0 up
ip link set bond0 master br0
```

### Network Service Control

```bash
# Init script locations
ls /etc/init.d/S4*network*
# S40network - Main network init
# S45bonding - Bonding configuration
# S46netwatch - Watchdog/auto-revert

# Restart sequence
/etc/init.d/S45bonding stop
/etc/init.d/S40network restart
/etc/init.d/S45bonding start

# Force revert to backup
/etc/init.d/S46netwatch revert
```

### Interface Status Commands

```bash
# All interface IPs
ip addr show

# Specific interface
ip addr show br0

# Interface statistics
ip -s link show ge0

# ARP table
ip neigh show

# Routing table
ip route show
```

---

## Additional Resources

- TuringPi Documentation: https://docs.turingpi.com/
- Linux Bonding Documentation: https://www.kernel.org/doc/Documentation/networking/bonding.txt
- Bridge Utils: https://wiki.linuxfoundation.org/networking/bridge

---

*Last updated: 2026-01-16*
