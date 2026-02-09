<div align="center">
  <img src="manager/logo.png" alt="DarkCore Logo" width="192" height="192">
  <br>
  <br>
  <pre>
██████╗  █████╗ ██████╗ ██╗  ██╗ ██████╗ ██████╗ ██████╗ ███████╗
██╔══██╗██╔══██╗██╔══██╗██║ ██╔╝██╔════╝██╔═══██╗██╔══██╗██╔════╝
██║  ██║███████║██████╔╝█████╔╝ ██║     ██║   ██║██████╔╝█████╗  
██║  ██║██╔══██║██╔══██╗██╔═██╗ ██║     ██║   ██║██╔══██╗██╔══╝  
██████╔╝██║  ██║██║  ██║██║  ██╗╚██████╗╚██████╔╝██║  ██║███████╗
╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚═╝  ╚═╝╚══════╝
  </pre>
  <h3>Advanced System Orchestration & Compatibility Layer</h3>
  <p>
    <b>High-Performance. Memory-Safe. Aesthetics.</b>
  </p>

# 🌌 DarkCore Manager v1.7.2

> **The Rust-Native Orchestrator for External Compatibility Layers.**
> *Zero Dependency. Zero Bloat. Pure Power.*

![Version](https://img.shields.io/badge/version-v1.7.1-00f3ff?style=for-the-badge&logo=rust)
![Status](https://img.shields.io/badge/status-STABLE-success?style=for-the-badge)
![License](https://img.shields.io/badge/license-MIT-lightgrey?style=for-the-badge)
  
  <p>
    <img src="https://img.shields.io/badge/Language-Rust_1.80+-orange?style=for-the-badge&logo=rust" alt="Rust">
    <img src="https://img.shields.io/badge/Platform-Windows_10%2F11-blue?style=for-the-badge&logo=windows" alt="Windows">
    <img src="https://img.shields.io/badge/Architecture-x64-lightgrey?style=for-the-badge">
    <img src="https://img.shields.io/badge/License-Educational-green?style=for-the-badge">
    <img src="https://img.shields.io/badge/Theme-Cyberpunk_2077-yellow?style=for-the-badge">
  </p>
</div>

---

<br>

> [!NOTE]
> **RESEARCH & INTEROPERABILITY DISCLAIMER**
>
> 1.  **Educational Sandbox**: **DarkCore Manager** is a technical demonstration of Rust UI patterns, IPC (Inter-Process Communication), and local file system management.
> 2.  **External Dependencies**: This tool acts solely as a **Launcher/Manager** for third-party tools (GreenLuma, Steamless, etc.). It does not contain, distribute, or modify their binaries.
> 3.  **No Proprietary Data**: This repository **does NOT** host any copyrighted game binaries or proprietary code. It operates strictly by managing local configuration text files (e.g., `AppList/*.txt`).
> 4.  **User Agency**: The user retains full control over their local environment. The author assumes **NO LIABILITY** for the usage of this tool or the behavior of third-party dependencies managed by it.

<br>

## 🚀 System Overview

**DarkCore Manager** redefines the local library management experience. Abandoning legacy script-based approaches, it introduces a **Rust-native architecture** designed for speed, safety, and visual immersion.

Acting as a sophisticated **Middleware Orchestrator**, it automates the complex interplay between the Client environment, external compatibility layers, and local configuration files, wrapping it all in a "God-Tier" interface.

## 🧠 Architecture: Under the Hood

* **Deterministic Configuration**: Dynamically builds and sorts the `AppList` directory structure, ensuring precise loading orders for external injectors.
* **Search & Indexing**: Powered by external Metadata APIs, enabling rapid retrieval of AppID information for local configuration.
* **Depot Management**: Parses Lua scripts to structurally align key-values into `config.vdf` and manages `depotcache` manifest placement for correct client recognition.
* **Profile Virtualization**: Overcomes legacy limitations by implementing a hot-swappable Profile System for `AppList` configurations.
* **Process Supervision**: Manages the lifecycle of child processes via native Win32 calls, ensuring clean startup and termination sequences.

**It doesn't just run commands. It governs the environment.**

## 🗝️ System Attributes: Core Modules
To achieve seamless interoperability, four components must work in unison. DarkCore orchestrates them all:

### 1. 🔓 Binary Preprocessor (Wrapper for Steamless)
* **Role**: **Automated File Preparation**.
* **Function**: Interfacing with the Steamless CLI to prepare executables for offline or sandboxed execution, ensuring compatibility with custom environments.
* **DarkCore Integration**: Fully automated workflow. Handles `.bak` creation, processing, and file restoration with 100% safety checks.

### 2. 🔑 Parameter Injection (Wrapper for GreenLuma)
* **Role**: **Environment Variable Management**.
* **Function**: Orchestrates the injection of specific AppIDs into the client's runtime context, leveraging legitimate "Family Sharing" protocols for extended library management.
* **DarkCore Integration**: Feeds the `AppList` configuration dynamically based on the active user profile.

### 3. 📡 Metadata Aggregation (Morrenus Integration)
* **Role**: **Manifest & Config Synchronization**.
* **Function**: Facilitates the retrieval of public **Manifests** and configuration scripts necessary for client validation.
* **DarkCore Integration**: Automates the alignment of Lua scripts and Manifests to ensure `config.vdf` and `depotcache` consistency.

### 4. NATIVE RUST DOWNLOADER


---

## ✨ Feature Matrix

### 🟢 Hybrid Operation Mode
DarkCore adapts to your environment in real-time.

| Mode              | Status          | Functionality                                                                                               |
| :---------------- | :-------------- | :---------------------------------------------------------------------------------------------------------- |
| **Authenticated** | **FULL ACCESS** | Complete API integration. Precise AppID resolution. Manifest/Lua synchronization enabled.                   |
| **Standard**      | **LOCAL ONLY**  | **Fallback to Public Store API**. Instant Search & DLC listings. Ideal for local Family Sharing management. |

> [!NOTE]
> **BYOL (Bring Your Own License)**: GreenLuma, Steamless, and API Keys are third-party resources. You must acquire and configure them independently.

### 🛡️ Integrity Validation
When a valid API connection is established, DarkCore enforces **Strict Validation**, preventing the configuration of invalid AppIDs. If the metadata doesn't exist, the configuration is rejected to maintain system stability.

### ⚡ Technical Highlights
* **Rust Native**: Compiled to machine code. Zero interpreter overhead. Only **18.5MB** standalone binary.
* **Project Neon UI**: Rebuilt with a "Glass & Glow" design language. Features fixed Sidebar navigation, adaptive layouts, and fade animations.
* **Smart Discovery**: Algorithms scan library folders to auto-fill installation paths, minimizing manual configuration.
* **Audio-Reactive**: Custom "Neon Wave" volume control with real-time spectrum visualization.
* **Secure Input**: API Key fields feature a "Glitch" security visualization.
* **Zero-Compromise Engineering**: The codebase compiles with **0 Warnings**, adhering to strict Rust 2024 standards.
* **Native Process Injection**: Utilizes advanced `QueueUserAPC` calls for stable, thread-safe module loading. This ensures seamless integration without the instability of legacy injection methods.
* **Profile Swapping**: Proprietary system allows for unlimited library configurations by hot-swapping `AppList` files.

### 🆕 v1.7.x Additions
* **⬇️ OTA Self-Update**: Automatic update checks on startup. One-click download & restart via GitHub Releases.
* **🎯 The Manifestor**: Surgical DLC selection modal with hierarchical view, search, and AppList slot protection.
* **🔄 WUDRM Auto-Recovery**: Detects "Update Required" flags and auto-downloads missing manifests.
* **🧠 F2P Intelligence**: Auto-detects Free-to-Play titles, displays "FREE" badges, and skips AppList injection.

---

<br>
<br>

<div align="center">

```
╔══════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                      ║
║     ░██████╗██╗░░░██╗░██████╗████████╗███████╗███╗░░░███╗  ██████╗░███████╗███████╗  ║
║     ██╔════╝╚██╗░██╔╝██╔════╝╚══██╔══╝██╔════╝████╗░████║  ██╔══██╗██╔════╝██╔════╝  ║
║     ╚█████╗░░╚████╔╝░╚█████╗░░░░██║░░░█████╗░░██╔████╔██║  ██║░░██║█████╗░░█████╗░░  ║
║     ░╚═══██╗░░╚██╔╝░░░╚═══██╗░░░██║░░░██╔══╝░░██║╚██╔╝██║  ██║░░██║██╔══╝░░██╔══╝░░  ║
║     ██████╔╝░░░██║░░░██████╔╝░░░██║░░░███████╗██║░╚═╝░██║  ██████╔╝███████╗██║░░░░░  ║
║     ╚═════╝░░░░╚═╝░░░╚═════╝░░░░╚═╝░░░╚══════╝╚═╝░░░░░╚═╝  ╚═════╝░╚══════╝╚═╝░░░░░  ║
║                                                                                      ║
╚══════════════════════════════════════════════════════════════════════════════════════╝
```

</div>

<br>

## 🏗️ System Architecture Deep Dive

<div align="center">

```
┌─────────────────────────────────────────────────────────────────────┐
│                        DarkCore Manager                             │
│                    ┌─────────────────────┐                          │
│                    │   egui Render Loop   │                          │
│                    │   (eframe + wgpu)    │                          │
│                    └────────┬────────────┘                          │
│                             │                                       │
│              ┌──────────────┼──────────────┐                        │
│              ▼              ▼              ▼                        │
│     ┌──────────────┐ ┌───────────┐ ┌────────────┐                  │
│     │   Sidebar    │ │  Panels   │ │   Modals   │                  │
│     │  Navigation  │ │           │ │            │                  │
│     │  + Logo Anim │ │ • Install │ │ • Manifest │                  │
│     │  + Profile   │ │ • Library │ │ • DLC Pick │                  │
│     │  + Audio     │ │ • Settings│ │ • Import   │                  │
│     │  + Updates   │ │ • About   │ │ • Delete   │                  │
│     └──────────────┘ └─────┬─────┘ │ • Family   │                  │
│                            │       │ • Download │                  │
│                            │       └────────────┘                  │
│              ┌─────────────┼─────────────┐                          │
│              ▼             ▼             ▼                          │
│     ┌──────────────┐ ┌──────────┐ ┌──────────────┐                 │
│     │  API Client  │ │  Vault   │ │  Injector    │                 │
│     │  (Morrenus)  │ │  Cache   │ │  (Win32 APC) │                 │
│     │  + Search    │ │  System  │ │  + Steamless │                 │
│     │  + Hierarchy │ │          │ │  + Goldberg  │                 │
│     │  + Manifests │ │  Lua +   │ │              │                 │
│     │  + Stats     │ │  Manifst │ │              │                 │
│     └──────┬───────┘ └────┬─────┘ └──────────────┘                 │
│            │              │                                         │
│            ▼              ▼                                         │
│     ┌──────────────────────────────┐                                │
│     │    Steam CDN / Local FS      │                                │
│     │    depotcache / steamapps    │                                │
│     └──────────────────────────────┘                                │
└─────────────────────────────────────────────────────────────────────┘
```

</div>

<br>

<table>
<tr>
<td width="50%">

### 🧬 Core Modules

| Module | Lines | Purpose |
|:-------|------:|:--------|
| `api.rs` | 849 | Morrenus API Client, Search, Hierarchy |
| `injector.rs` | — | Win32 `QueueUserAPC` DLL injection |
| `vault.rs` | 289 | Intelligent manifest caching layer |
| `vdf_injector.rs` | — | VDF config & `depotcache` manager |
| `watcher.rs` | 283 | Background update detector |
| `goldberg.rs` | — | Goldberg Emulator integration |
| `steamless.rs` | — | Steamless CLI automation |
| `game_path.rs` | — | Multi-library Steam path detection |

</td>
<td width="50%">

### 🎨 UI Architecture

| Layer | Files | Role |
|:------|------:|:-----|
| **Panels** | 4 | Install, Library, Settings, About |
| **Modals** | 7 | Manifestor, DLC Picker, Import, Delete, Family/Download, Install, Download Method |
| **Components** | 4 | Sidebar, Terminal, Progress, Covers |
| **Engine** | 3 | Render loop, State, Theme |

</td>
</tr>
</table>

<br>

---

<br>

<div align="center">

```
╔══════════════════════════════════════════════════════════════════╗
║                                                                  ║
║        ██╗███╗░░██╗░██████╗████████╗░█████╗░██╗░░░░░██╗░░░░░    ║
║        ██║████╗░██║██╔════╝╚══██╔══╝██╔══██╗██║░░░░░██║░░░░░    ║
║        ██║██╔██╗██║╚█████╗░░░░██║░░░███████║██║░░░░░██║░░░░░    ║
║        ██║██║╚████║░╚═══██╗░░░██║░░░██╔══██║██║░░░░░██║░░░░░    ║
║        ██║██║░╚███║██████╔╝░░░██║░░░██║░░██║███████╗███████╗    ║
║        ╚═╝╚═╝░░╚══╝╚═════╝░░░░╚═╝░░░╚═╝░░╚═╝╚══════╝╚══════╝    ║
║                                                                  ║
║             F  L  O  W  S    &    P  R  O  T  O  C  O  L  S     ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
```

</div>

<br>

### 🎮 The Installation Pipeline

DarkCore's installation system is a multi-stage pipeline that adapts to user intent. Every path converges through a **unified decision tree**, ensuring consistency regardless of the entry point.

<div align="center">

```
  ╔═══════════════════════════════════════════════════════════════════════╗
  ║                                                                       ║
  ║                    ┌──────────────┐    ┌──────────────┐               ║
  ║                    │  🔍 SEARCH   │    │  📂 IMPORT   │               ║
  ║                    │   Catalog    │    │   ZIP File   │               ║
  ║                    └──────┬───────┘    └──────┬───────┘               ║
  ║                           │                   │                       ║
  ║                           ▼                   ▼                       ║
  ║                    ┌──────────────────────────────────┐               ║
  ║                    │      📦 THE MANIFESTOR           │               ║
  ║                    │                                  │               ║
  ║                    │  • Fetch Game Hierarchy (API)    │               ║
  ║                    │  • Display Base Game + All DLCs  │               ║
  ║                    │  • Smart Slot Counter (130 max)  │               ║
  ║                    │  • Select / Deselect / Filter    │               ║
  ║                    │                                  │               ║
  ║                    │  ┌────────────────────────────┐  │               ║
  ║                    │  │  🔄 Morrenus Failover:     │  │               ║
  ║                    │  │  Vault Cache → API → Lua   │  │               ║
  ║                    │  │  (Token-Saving Protocol)   │  │               ║
  ║                    │  └────────────────────────────┘  │               ║
  ║                    └──────────────┬───────────────────┘               ║
  ║                                   │                                   ║
  ║                                   ▼                                   ║
  ║                    ┌──────────────────────────────────┐               ║
  ║                    │  🎮 FAMILY SHARED or ⬇ DOWNLOAD? │               ║
  ║                    └───────┬──────────────────┬───────┘               ║
  ║                            │                  │                       ║
  ║                    ┌───────▼───────┐  ┌───────▼────────┐             ║
  ║                    │  👨‍👩‍👧 FAMILY   │  │  💾 LIBRARY     │             ║
  ║                    │   SHARED      │  │   SELECTION     │             ║
  ║                    │               │  │                 │             ║
  ║                    │ Write AppList │  │ Select Drive +  │             ║
  ║                    │ (Base + DLCs) │  │ Install Folder  │             ║
  ║                    │ No Downloads  │  │ Smart Auto-Fill │             ║
  ║                    │ No API Calls  │  │ Scan Existing   │             ║
  ║                    │               │  └────────┬────────┘             ║
  ║                    │   ✅ DONE     │           │                      ║
  ║                    └───────────────┘           ▼                      ║
  ║                                    ┌───────────────────────┐         ║
  ║                                    │  ☁️ STEAM  or  ⚡ DIRECT │         ║
  ║                                    └─────┬───────────┬─────┘         ║
  ║                                          │           │               ║
  ║                                  ┌───────▼──┐  ┌─────▼────────┐     ║
  ║                                  │ Steam    │  │ Direct       │     ║
  ║                                  │ Protocol │  │ Download     │     ║
  ║                                  │          │  │              │     ║
  ║                                  │ • Inject │  │ • Parse Lua  │     ║
  ║                                  │ • ACF    │  │ • Auth Anon  │     ║
  ║                                  │ • Launch │  │ • Fetch CDN  │     ║
  ║                                  │ • WUDRM  │  │ • Decrypt    │     ║
  ║                                  └──────────┘  └──────────────┘     ║
  ║                                                                       ║
  ╚═══════════════════════════════════════════════════════════════════════╝
```

</div>

<br>

### 🧊 The Vault — Intelligent Caching Layer

The Vault is DarkCore's built-in **token-saving** caching system. Every API interaction passes through the Vault first, dramatically reducing external calls and enabling **offline operation** for previously configured games.

<div align="center">

```
                        ┌────────────────────┐
                        │  📡 API Request    │
                        └─────────┬──────────┘
                                  │
                                  ▼
                     ╔════════════════════════╗
                     ║    🧊 VAULT CHECK     ║
                     ║                        ║
                     ║  Vault/{AppID}/        ║
                     ║  ├── {AppID}.lua       ║
                     ║  ├── {Depot}_{GID}     ║
                     ║  │   .manifest         ║
                     ║  └── {AppID}.zip       ║
                     ╚════════╤═══════════════╝
                              │
                    ┌─────────┴─────────┐
                    │                   │
               HIT  ▼              MISS ▼
          ┌──────────────┐    ┌──────────────┐
          │ ✅ Use Local │    │ 📡 Download  │
          │  (0 Tokens)  │    │  from API    │
          └──────────────┘    └──────┬───────┘
                                     │
                                     ▼
                              ┌──────────────┐
                              │ 💾 Save to   │
                              │    Vault     │
                              │ (Extracted)  │
                              └──────────────┘
```

</div>

The Vault also powers **manifest version verification** — comparing local `{depot_id}_{gid}.manifest` filenames against expected GIDs from the API, enabling automatic detection of outdated content.

<br>

---

<br>

<div align="center">

```
╔══════════════════════════════════════════════════════════════════╗
║                                                                  ║
║     ██████╗░██████╗░███████╗██████╗░███████╗░██████╗░░██████╗   ║
║     ██╔══██╗██╔══██╗██╔════╝██╔══██╗██╔════╝██╔═══██╗██╔════╝   ║
║     ██████╔╝██████╔╝█████╗░░██████╔╝█████╗░░██║██╗██║╚█████╗░   ║
║     ██╔═══╝░██╔══██╗██╔══╝░░██╔══██╗██╔══╝░░╚██████╔╝░╚═══██╗   ║
║     ██║░░░░░██║░░██║███████╗██║░░██║███████╗░╚═██╔═╝░██████╔╝   ║
║     ╚═╝░░░░░╚═╝░░╚═╝╚══════╝╚═╝░░╚═╝╚══════╝░░╚═╝░░░╚═════╝   ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
```

</div>

<br>

> [!IMPORTANT]
> ### ⚠️ GreenLuma Version Requirement
> 
> DarkCore requires **GreenLuma 2025 version 1.7.0** specifically.
> 
> Starting from version **1.7.1**, the developer disabled the download functionality that DarkCore's pipeline depends on. Using any version newer than 1.7.0 will result in **broken download flows** and incomplete installations.
> 
> **Ensure you have `GreenLuma_2025_x64.dll` from the v1.7.0 release.**

<br>

### 📋 Prerequisites Checklist

<table>
<tr>
<td width="50%">

#### 🔧 Build Requirements

| Requirement | Version |
|:------------|:--------|
| Rust Toolchain | `≥ 1.80` via [rustup.rs](https://rustup.rs/) |
| Windows SDK | Latest |
| Git | Latest |
| Target | `x86_64-pc-windows-msvc` |

</td>
<td width="50%">

#### 🧩 Runtime Dependencies (BYOL)

| Component | Required | Notes |
|:----------|:--------:|:------|
| GreenLuma 2025 | ✅ | **v1.7.0 ONLY** |
| Steamless CLI | ⬜ Optional | For DRM removal |
| Morrenus API Key | ⬜ Optional | For full API access |
| Goldberg Emulator | ⬜ Optional | For offline mode |

</td>
</tr>
</table>

<br>

### 🛠️ Build Sequence

```powershell
# ── Clone ──────────────────────────────────────────────────────────
git clone https://github.com/hkmodd/DarkCore-Manager.git
cd DarkCore-Manager

# ── Assets (Auto-embedded at compile time) ─────────────────────────
#    Logo:  manager/logo.png    → Baked into binary
#    Icon:  manager/icon.ico    → Windows executable icon

# ── Compile ────────────────────────────────────────────────────────
cargo build --release

# ── Output ─────────────────────────────────────────────────────────
#    target/release/darkcore-manager.exe    (~18.5 MB standalone)
```

<br>

---

<br>

<div align="center">

```
╔══════════════════════════════════════════════════════════════════╗
║                                                                  ║
║        ░██████╗░██╗░░░██╗██╗██████╗░███████╗                    ║
║        ██╔════╝░██║░░░██║██║██╔══██╗██╔════╝                    ║
║        ██║░░██╗░██║░░░██║██║██║░░██║█████╗░░                    ║
║        ██║░░╚██╗██║░░░██║██║██║░░██║██╔══╝░░                    ║
║        ╚██████╔╝╚██████╔╝██║██████╔╝███████╗                    ║
║        ░╚═════╝░░╚═════╝░╚═╝╚═════╝░╚══════╝                    ║
║                                                                  ║
║          O  P  E  R  A  T  I  O  N  A  L    G  U  I  D  E       ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
```

</div>

<br>

### ⚙️ Phase 1 — Environment Mapping

On first boot, navigate to **SETTINGS** to initialize your environment:

<table>
<tr>
<td width="60">🎮</td>
<td><b>Steam Root</b></td>
<td>Directory containing <code>steam.exe</code>. Auto-detected from Windows Registry.</td>
</tr>
<tr>
<td>🔑</td>
<td><b>GreenLuma Path</b></td>
<td>Folder with <code>GreenLuma_2025_x64.dll</code>. <b>Must be v1.7.0.</b></td>
</tr>
<tr>
<td>🔓</td>
<td><b>Steamless Path</b></td>
<td>(Optional) Path to <code>Steamless.CLI.exe</code> for DRM binary preprocessing.</td>
</tr>
<tr>
<td>📡</td>
<td><b>API Key</b></td>
<td>(Optional) Morrenus API key for full metadata access. Displayed with glitch-style security visualization.</td>
</tr>
</table>

> [!TIP]
> DarkCore auto-detects your Steam installation path from the Windows Registry on first boot. Multi-library setups across multiple drives are fully supported.

<br>

### ⚙️ Phase 2 — The Execution Cycle

<table>
<tr><td width="50">

**`01`**

</td><td>

**SEARCH** — Navigate to the Install tab. Type a game name or AppID. Results appear in a responsive grid with live cover art pulled from Steam CDN. Hover over any card for a **premium detail popup** with Metacritic scores, genres, platforms, and descriptions.

</td></tr>
<tr><td>

**`02`**

</td><td>

**INSTALL** — Click any uninstalled game to open **The Manifestor**. It fetches the full game hierarchy including all DLCs, displays slot usage against the 130-entry AppList limit, and lets you surgically select which content to include.

</td></tr>
<tr><td>

**`03`**

</td><td>

**CHOOSE** — After DLC selection, choose your path: **Family Shared** (AppList-only, zero downloads, zero API calls) or **Download** (full content acquisition). Family Shared is instant — perfect for games shared by another account.

</td></tr>
<tr><td>

**`04`**

</td><td>

**DEPLOY** — If downloading, select your target Steam Library drive and installation folder (auto-detected with smart scan). Then choose: **Steam Protocol** (standard client-driven download) or **Direct Download** (experimental CDN-direct acquisition that bypasses the client entirely).

</td></tr>
<tr><td>

**`05`**

</td><td>

**PLAY** — Click any installed game card to launch. DarkCore handles the full injection chain: `AppList` write → GreenLuma configuration → Stealth Mode → Process injection via `QueueUserAPC` → Game launch. If Steam isn't running, it starts with injection in a single operation.

</td></tr>
</table>

<br>

### ⚙️ Phase 3 — Library Management

The **Library** tab is your command center for all installed content:

<table>
<tr><td width="50">🔄</td><td><b>Profiles</b> — Hot-swap between unlimited AppList configurations. Create, load, save, and delete profiles. Each profile is a complete snapshot of your managed library.</td></tr>
<tr><td>📡</td><td><b>Update Watcher</b> — Automatic background BuildID monitoring. Detects when games receive updates and flags them for manifest re-download. Manual check available via the toolbar.</td></tr>
<tr><td>🔄</td><td><b>WUDRM Recovery</b> — When Steam flags a game as "Update Required", DarkCore's <b>Wudrm Auto-Recovery</b> engine silently downloads the correct manifests from SteamCMD public API and places them in <code>depotcache</code>.</td></tr>
<tr><td>🔓</td><td><b>Steamless Integration</b> — One-click DRM removal for offline play. Automatic backup creation with <code>.bak</code> extension before processing.</td></tr>
<tr><td>🎮</td><td><b>Goldberg Emulator</b> — Generate Goldberg configurations per-game with custom username and SteamID. Supports both x86 and x64 targeting.</td></tr>
<tr><td>❌</td><td><b>Clean Delete</b> — Remove games with full DLC association detection. Automatically finds and offers to remove all related DLC entries alongside the base game.</td></tr>
</table>

<br>

---

<br>

<div align="center">

```
╔══════════════════════════════════════════════════════════════════╗
║                                                                  ║
║     ████████╗███████╗░█████╗░██╗░░██╗  ░██████╗████████╗░█████╗  ║
║     ╚══██╔══╝██╔════╝██╔══██╗██║░░██║  ██╔════╝╚══██╔══╝██╔══██╗ ║
║     ░░░██║░░░█████╗░░██║░░╚═╝███████║  ╚█████╗░░░░██║░░░███████║ ║
║     ░░░██║░░░██╔══╝░░██║░░██╗██╔══██║  ░╚═══██╗░░░██║░░░██╔══██║ ║
║     ░░░██║░░░███████╗╚█████╔╝██║░░██║  ██████╔╝░░░██║░░░██║░░██║ ║
║     ░░░╚═╝░░░╚══════╝░╚════╝░╚═╝░░╚═╝  ╚═════╝░░░░╚═╝░░░╚═╝░░╚═╝ ║
║                                                                  ║
║              T  E  C  H  N  I  C  A  L    S  T  A  C  K          ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
```

</div>

<br>

<table>
<tr>
<td width="33%" valign="top">

### 🦀 Language & Runtime
| | |
|:--|:--|
| **Language** | Rust 2024 Edition |
| **Async** | Tokio multi-threaded |
| **GUI** | egui + eframe + wgpu |
| **HTTP** | reqwest (TLS) |
| **Binary** | ~18.5 MB standalone |
| **Warnings** | 0 |

</td>
<td width="33%" valign="top">

### 🎨 UI Design System
| | |
|:--|:--|
| **Theme** | Cyberpunk Neon |
| **Palette** | Obsidian / Cyan / Pink |
| **Font** | System + Monospace |
| **Animation** | Lerp-based, 250ms |
| **Layout** | Fixed Sidebar + Adaptive Grid |
| **Modals** | 7 overlay windows |

</td>
<td width="33%" valign="top">

### 🔒 Security
| | |
|:--|:--|
| **Injection** | Win32 QueueUserAPC |
| **API Keys** | Glitch-masked display |
| **TLS** | reqwest native |
| **Memory** | Rust ownership model |
| **Config** | Local JSON only |
| **Network** | No telemetry |

</td>
</tr>
</table>

<br>

### 📦 Dependency Highlights

```toml
[dependencies]
eframe       = "0.29"      # Native GPU-accelerated UI framework
egui_extras  = "0.29"      # Image loaders and advanced widgets
tokio        = "1"          # Async runtime for concurrent operations
reqwest      = "0.12"       # HTTP client with TLS support
serde_json   = "1"          # Configuration serialization
image        = "0.25"       # Cover art processing and resizing
zip          = "2.6"        # Morrenus ZIP archive extraction
rodio        = "0.19"       # Audio playback engine
rfd          = "0.15"       # Native file dialogs
regex        = "1"          # Pattern matching for VDF/ACF parsing
glob         = "0.3"        # File system pattern matching
open          = "5"          # OS-native URL/file opening
ico          = "0.3"        # Windows icon embedding
winapi       = "0.3"        # Win32 API for process injection
```

<br>

---

<br>

<div align="center">

```
╔══════════════════════════════════════════════════════════════════╗
║                                                                  ║
║      ██████╗░██████╗░░█████╗░░░░░░██╗███████╗░█████╗░████████╗  ║
║      ██╔══██╗██╔══██╗██╔══██╗░░░░░██║██╔════╝██╔══██╗╚══██╔══╝  ║
║      ██████╔╝██████╔╝██║░░██║░░░░░██║█████╗░░██║░░╚═╝░░░██║░░░  ║
║      ██╔═══╝░██╔══██╗██║░░██║██╗░░██║██╔══╝░░██║░░██╗░░░██║░░░  ║
║      ██║░░░░░██║░░██║╚█████╔╝╚█████╔╝███████╗╚█████╔╝░░░██║░░░  ║
║      ╚═╝░░░░░╚═╝░░╚═╝░╚════╝░░╚════╝░╚══════╝░╚════╝░░░░╚═╝░░░  ║
║                                                                  ║
║                  S  T  R  U  C  T  U  R  E                       ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
```

</div>

<br>

```
DarkCore-Manager/
│
├── 📄 Cargo.toml                    # Workspace manifest
├── 📄 relationships.json            # AppID ↔ DLC relational map
│
└── manager/                         # ━━━ MAIN CRATE ━━━
    ├── 📄 Cargo.toml
    ├── 📄 build.rs                  # Compile-time resource embedding
    ├── 🎨 logo.png                  # Embedded at compile time
    ├── 🎨 icon.ico                  # Windows executable icon
    │
    ├── core_data/                   # 🔊 Audio assets (obfuscated)
    │
    └── src/
        ├── main.rs                  # ─── Entry Point ──────────────
        │
        │                            # ─── Core Systems ─────────────
        ├── api.rs                   # Morrenus API Client
        ├── config.rs                # AppConfig persistence
        ├── cache.rs                 # Game metadata cache
        ├── vault.rs                 # 🧊 Intelligent Vault system
        ├── watcher.rs               # Background update monitor
        ├── updater.rs               # OTA self-update via GitHub
        │
        │                            # ─── Steam Integration ────────
        ├── game_path.rs             # Multi-library path detection
        ├── registry.rs              # Windows Registry operations
        ├── app_list.rs              # GreenLuma AppList management
        ├── profiles.rs              # Profile serialization
        ├── vdf_injector.rs          # config.vdf + depotcache ops
        ├── manifest_downloader.rs   # SteamCMD manifest fetcher
        │
        │                            # ─── Tools ────────────────────
        ├── injector.rs              # Win32 QueueUserAPC injection
        ├── goldberg.rs              # Goldberg Emulator generator
        ├── steamless.rs             # Steamless CLI automation
        │
        ├── direct_download/         # ─── Direct Download Engine ───
        │   ├── mod.rs
        │   ├── downloader.rs        #   CDN chunk downloader
        │   ├── lua_parser.rs        #   Morrenus Lua script parser
        │   ├── manifest.rs          #   Steam manifest decoder
        │   └── state.rs             #   Download state machine
        │
        ├── steam/                   # ─── Steam Manifest Layer ─────
        │   ├── mod.rs
        │   └── manifest.rs          #   ACF file operations
        │
        └── ui/                      # ─── User Interface ───────────
            ├── mod.rs
            ├── app.rs               #   App initialization + impl
            ├── render.rs            #   Main render orchestrator
            ├── state.rs             #   DarkCoreApp struct (40+ fields)
            ├── theme.rs             #   Cyberpunk color palette
            ├── helpers.rs           #   Shared utility functions
            ├── covers.rs            #   Cover art URL resolution
            ├── install_logic.rs     #   Installation pipeline engine
            ├── watcher.rs           #   Watcher UI integration
            │
            ├── panels/              #   ── Tab Panels ──────────
            │   ├── install.rs       #     Search + Game Grid + Hover
            │   ├── library.rs       #     Profiles + Game Management
            │   ├── settings.rs      #     Configuration + API Stats
            │   └── about.rs         #     Matrix Rain easter egg
            │
            ├── modals/              #   ── Modal Windows ───────
            │   ├── manifestor.rs    #     DLC Hierarchy Selector
            │   ├── dlc_picker.rs    #     Legacy DLC Picker
            │   ├── family_or_download.rs  # Family/Download Choice
            │   ├── install_modal.rs #     Library Selection
            │   ├── download_method.rs #   Steam vs Direct
            │   ├── import_zip.rs    #     ZIP Import Handler
            │   └── delete.rs        #     Delete Confirmation
            │
            └── components/          #   ── Reusable Widgets ────
                ├── sidebar.rs       #     Navigation + Logo + Audio
                ├── terminal.rs      #     System Log Console
                └── progress.rs      #     Download Progress Bar
```

<br>

---

<br>

<div align="center">

```
╔══════════════════════════════════════════════════════════════════════════╗
║                                                                          ║
║     ██████╗ ██╗██████╗ ███████╗ ██████╗████████╗                        ║
║     ██╔══██╗██║██╔══██╗██╔════╝██╔════╝╚══██╔══╝                        ║
║     ██║  ██║██║██████╔╝█████╗  ██║      ░░██║░░░                        ║
║     ██║  ██║██║██╔══██╗██╔══╝  ██║      ░░██║░░░                        ║
║     ██████╔╝██║██║  ██║███████╗╚██████╗ ░░██║░░░                        ║
║     ╚═════╝ ╚═╝╚═╝  ╚═╝╚══════╝ ╚═════╝ ░░╚═╝░░░                        ║
║                                                                          ║
║     ██████╗  ██████╗ ██╗    ██╗███╗   ██╗██╗      ██████╗  █████╗ ██████╗║
║     ██╔══██╗██╔═══██╗██║    ██║████╗  ██║██║     ██╔═══██╗██╔══██╗██╔══██║
║     ██║  ██║██║   ██║██║ █╗ ██║██╔██╗ ██║██║     ██║   ██║███████║██║  ██║
║     ██║  ██║██║   ██║██║███╗██║██║╚██╗██║██║     ██║   ██║██╔══██║██║  ██║
║     ██████╔╝╚██████╔╝╚███╔███╔╝██║ ╚████║███████╗╚██████╔╝██║  ██║██████╔║
║     ╚═════╝  ╚═════╝  ╚══╝╚══╝ ╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚═════╝║
║                                                                          ║
║              E  N  G  I  N  E    I  N  T  E  R  N  A  L  S               ║
║                                                                          ║
╚══════════════════════════════════════════════════════════════════════════╝
```

</div>

<br>

The **Direct Download** engine is DarkCore's experimental CDN-direct acquisition system. It bypasses the Steam client entirely, downloading game content directly from Valve's content servers.

### How It Works

<div align="center">

```
  ┌──────────────────────────────────────────────────────────────────┐
  │                                                                  │
  │    ┌───────────┐     ┌───────────┐     ┌───────────────────┐    │
  │    │ 1. FETCH  │     │ 2. PARSE  │     │ 3. AUTHENTICATE   │    │
  │    │           │     │           │     │                   │    │
  │    │ Download  │────▶│ Extract   │────▶│ Anonymous Steam   │    │
  │    │ Morrenus  │     │ Lua +     │     │ Login via         │    │
  │    │ ZIP       │     │ Manifests │     │ steam-vent        │    │
  │    └───────────┘     └───────────┘     └─────────┬─────────┘    │
  │                                                  │              │
  │    ┌───────────────────────────────────────────────              │
  │    │                                                             │
  │    ▼                                                             │
  │    ┌───────────────────┐     ┌───────────────────┐              │
  │    │ 4. DOWNLOAD       │     │ 5. DECRYPT        │              │
  │    │                   │     │                   │              │
  │    │ Fetch chunks      │────▶│ AES-256-ECB       │              │
  │    │ from Steam CDN    │     │ Depot key from    │              │
  │    │ (Parallel I/O)    │     │ Morrenus Lua      │              │
  │    └───────────────────┘     └─────────┬─────────┘              │
  │                                        │                        │
  │                                        ▼                        │
  │                              ┌───────────────────┐              │
  │                              │ 6. FINALIZE       │              │
  │                              │                   │              │
  │                              │ • Write ACF       │              │
  │                              │ • Place Manifests │              │
  │                              │ • Inject AppList  │              │
  │                              │ • Update VDF      │              │
  │                              └───────────────────┘              │
  │                                                                  │
  └──────────────────────────────────────────────────────────────────┘
```

</div>

<br>

<table>
<tr>
<td width="50%" valign="top">

#### 🔑 Key Technical Details

- **Chunk-Level Parallelism** — Multiple depot chunks downloaded concurrently via `tokio` async tasks
- **AES-256-ECB Decryption** — Depot keys extracted from Morrenus Lua scripts decrypt chunk payloads
- **Manifest Parsing** — Custom binary parser for Steam's protobuf-based `.manifest` format
- **Progress Tracking** — Real-time speed calculation with byte-level accuracy, displayed in the UI progress bar

</td>
<td width="50%" valign="top">

#### 🛡️ Safety Guarantees

- **Vault Integration** — Downloaded manifests and Lua scripts are immediately cached in the Vault
- **Atomic Writes** — File operations use temporary paths to prevent corruption on failure
- **State Machine** — `Idle → Initializing → FetchingManifest → Downloading → Decrypting → Verifying → Finalizing → Completed`
- **Error Recovery** — Each stage reports to the UI terminal with full error context

</td>
</tr>
</table>

<br>

---

<br>

<div align="center">

```
╔══════════════════════════════════════════════════════════════════╗
║                                                                  ║
║        ██╗░░░░░██╗███╗░░██╗███████╗░█████╗░░██████╗░███████╗    ║
║        ██║░░░░░██║████╗░██║██╔════╝██╔══██╗██╔════╝░██╔════╝    ║
║        ██║░░░░░██║██╔██╗██║█████╗░░███████║██║░░██╗░█████╗░░    ║
║        ██║░░░░░██║██║╚████║██╔══╝░░██╔══██║██║░░╚██╗██╔══╝░░    ║
║        ███████╗██║██║░╚███║███████╗██║░░██║╚██████╔╝███████╗    ║
║        ╚══════╝╚═╝╚═╝░░╚══╝╚══════╝╚═╝░░╚═╝░╚═════╝░╚══════╝    ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
```

</div>

<br>

```
DarkCore Manager
│
├── v1.0.0  ──  Initial Release. Search + AppList + Basic Inject.
├── v1.1.0  ──  Profile System. Multi-config hot-swapping.
├── v1.2.0  ──  Steamless Integration. Automated DRM preprocessing.
├── v1.3.0  ──  Goldberg Emulator. Offline play configuration.
├── v1.4.0  ──  Direct Download Engine. CDN-direct acquisition.
├── v1.5.0  ──  Project Neon UI. Complete visual overhaul.
├── v1.6.0  ──  Vault System. Intelligent manifest caching.
│
└── v1.7.0  ──  ⭐ CURRENT
    │
    ├── ⬇️  OTA Self-Update via GitHub Releases
    ├── 🎯  The Manifestor — Surgical DLC selection
    ├── 🔄  WUDRM Auto-Recovery — Manifest repair engine
    ├── 🧠  F2P Intelligence — Free content detection
    ├── 👨‍👩‍👧  Family Shared Mode — AppList-only install (0 downloads)
    ├── 📂  ZIP Import — Offline manifest import
    ├── 🧊  Vault Verification — GID-based version checking
    ├── 🎴  Premium Hover Cards — Steam Store detail popups
    ├── 📡  Background Watcher — BuildID update monitoring
    └── 🎵  Audio System — Embedded ambient soundtrack
```

<br>

---

<br>

<div align="center">

```
╔═════════════════════════════════════════════════════╗
║                                                     ║
║        ░█████╗░██████╗░███████╗██████╗░██╗████████╗ ║
║        ██╔══██╗██╔══██╗██╔════╝██╔══██╗██║╚══██╔══╝ ║
║        ██║░░╚═╝██████╔╝█████╗░░██║░░██║██║░░░██║░░░ ║
║        ██║░░██╗██╔══██╗██╔══╝░░██║░░██║██║░░░██║░░░ ║
║        ╚█████╔╝██║░░██║███████╗██████╔╝██║░░░██║░░░ ║
║        ░╚════╝░╚═╝░░╚═╝╚══════╝╚═════╝░╚═╝░░░╚═╝░░░ ║
║                                                     ║
╚═════════════════════════════════════════════════════╝
```

</div>

<br>

<div align="center">

Powered by the Rust ecosystem and the open-source community.

<br>

**Third-Party Acknowledgments** — GreenLuma by [Steam006](https://cs.rin.ru/forum/viewtopic.php?f=29&t=103709) · Steamless by [atom0s](https://github.com/atom0s/Steamless)
Goldberg Emulator by [Mr. Goldberg](https://gitlab.com/Mr_Goldberg/goldberg_emulator)
Morrenus API by [manifest.morrenus.xyz](https://manifest.morrenus.xyz/) · egui by [emilk](https://github.com/emilk/egui)

<br>
<br>

<img src="https://img.shields.io/badge/Built_with-Rust_&_Obsession-red?style=for-the-badge&logo=rust&logoColor=white">

<br>

<pre>
                        ██████╗  █████╗ ██████╗ ██╗  ██╗
                        ██╔══██╗██╔══██╗██╔══██╗██║ ██╔╝
                        ██║  ██║███████║██████╔╝█████╔╝ 
                        ██║  ██║██╔══██║██╔══██╗██╔═██╗ 
                        ██████╔╝██║  ██║██║  ██║██║  ██╗
                        ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝
                        
            "Wake up, Samurai. We have a compiled language to burn."
</pre>

<br>

[![Star History](https://img.shields.io/badge/⭐_Star_this_repo-00f3ff?style=for-the-badge)](#)

</div>
