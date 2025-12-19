<div align="center">
  <img src="logo.png" alt="DarkCore Logo" width="128" height="128">
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
  <h3>System Orchestration & Compatibility Layer</h3>
  <p>
    <b>High-Performance. Memory-Safe. Cyberpunk Aesthetics.</b>
  </p>
  
  <p>
    <img src="https://img.shields.io/badge/Language-Rust_1.80+-orange?style=for-the-badge&logo=rust" alt="Rust">
    <img src="https://img.shields.io/badge/Platform-Windows_10%2F11-blue?style=for-the-badge&logo=windows" alt="Windows">
    <img src="https://img.shields.io/badge/Architecture-x64-lightgrey?style=for-the-badge">
    <img src="https://img.shields.io/badge/License-Educational-green?style=for-the-badge">
    <img src="https://img.shields.io/badge/Theme-Cyberpunk_2077-yellow?style=for-the-badge">
  </p>

  <p>
    <a href="#features">Features</a> •
    <a href="#architecture">Architecture</a> •
    <a href="#compilation">Compilation</a> •
    <a href="#disclaimer">Disclaimer</a>
  </p>
</div>

---

<br>

> [!CAUTION]
> **LEGAL DISCLAIMER & ZERO LIABILITY**
>
> 1.  **Independent Research**: **DarkCore Manager** is an independent project created solely for educational purposes, demonstrating advanced Rust UI patterns and process orchestration.
> 2.  **No Affiliation**: This software is **NOT** affiliated with, endorsed by, or connected to Valve Corporation, Steam, GreenLuma, Steamless, or Morrenus.
> 3.  **No Proprietary Data**: This tool **does NOT** contain, distribute, or host any copyrighted game binaries or proprietary code. It operates strictly by managing local configuration text files (e.g., `AppList/*.txt`).
> 4.  **User Responsibility**: The user assumes full responsibility for compliance with all applicable Terms of Service and local laws. The author assumes **NO LIABILITY** for bans, data loss, or system instability.

<br>

## 🚀 System Overview

**DarkCore Manager** redefines the compatibility layer experience. Abandoning legacy Python scripts, it introduces a **Rust-native architecture** designed for speed, safety, and visual immersion.

Acting as a sophisticated **Middleware Orchestrator**, it automates the complex interplay between Steam, file systems, and injection tools, wrapping it all in a "God-Tier" interface.

## 🧠 Architecture: Under the Hood

Unlike basic launchers, DarkCore operates as a state-aware system supervisor:

*   **AppID Context Switching**: Generates context-aware `AppId.txt` files to guide injection targets.
*   **AppList Sequencing**: Dynamically builds and sorts the `AppList` directory structure (`0.txt`, `1.txt`...), ensuring deterministic loading of entitlements by GreenLuma.
*   **License Injection (config.vdf)**: Parses Lua entitlement scripts from Morrenus to surgically inject `DecryptionKey`s into Steam's `config/config.vdf`, enabling authorized unlocking of encrypted depots.
*   **Manifest Deployment**: Extracts official signed Steam Manifests (`.manifest`) directly into the `depotcache` directory, allowing Steam to recognize and download game files as if owned.
*   **Process Orchestration**: Manages the lifecycle of `Steam.exe`, `DLLInjector.exe`, and game processes via native Win32 calls.

**It doesn't just run commands. It manages the environment.**

---

## ✨ Feature Matrix

### 🟢 Hybrid API Core
DarkCore adapts to your environment in real-time.

| Mode          | Status       | Functionality                                                                                                        |
| :------------ | :----------- | :------------------------------------------------------------------------------------------------------------------- |
| **Valid Key** | **UNLOCKED** | Full Morrenus API integration. Precise AppID resolution. Manifest downloads enabled. VDF Injection active.           |
| **No Key**    | **FALLBACK** | **Silent Fallback to Public Steam Store**. Instant Search & DLC listings via public API. Perfect for Family Sharing. |

> [!NOTE]
> **External Resources**: GreenLuma, Steamless, and API Keys are third-party resources. You must acquire them independently.

### 🛡️ Smart "Strict Check"
When a valid key is detected, DarkCore enforces **Strict Validation**, preventing the installation of broken or unsupported AppIDs. If it's not in the database, it doesn't touch your disk.

### ⚡ Technical Highlights
*   **Rust Native**: Compiled to machine code. Zero interpreter overhead. 15.1MB standalone binary.
*   **Cyberpunk Interface**: Built with `egui` (Immediate Mode GUI). High-FPS rendering, custom styling, responsive layout.
*   **Limit Bypass**: Proprietary **Profile System** allows unlimited libraries by hot-swapping `AppList` configurations.
*   **Steamless Automation**: Integrated GUI for `Steamless CLI` with **Auto-Backup** (`.bak`), **Auto-Rename**, and **Smart Detection**.

---

## 🛠️ Compilation Source

We believe in transparency. Build it yourself.

### Prerequisites
*   [**Rust Toolchain (rustup)**](https://rustup.rs/)
*   **Git** & **Windows SDK**

### Build Sequence

1.  **Clone Repository**
    ```powershell
    git clone https://github.com/hkmodd/DarkCore-Manager
    cd <foldername>
    ```

2.  **Asset Injection (Optional)**
    *   Place your `icon.ico` in the root directory. The build system (`build.rs`) will fuse it into the executable resource table.

3.  **Compile Release**
    ```powershell
    cargo build --release
    ```
    *Target Artifact: `target/release/darkcore-greenluma.exe`*

---

## ⚙️ Configuration Protocol

### 1. Initialization
Upon first boot, the system validates paths. Navigate to **SETTINGS** to map your environment:
1.  **Steam Path**: Root directory (e.g., `C:\Program Files (x86)\Steam`).
2.  **GreenLuma Path**: Directory containing `DLLInjector.exe`.
3.  **Steamless Path**: Path to `Steamless.CLI.exe` (Vital for DRM removal).
4.  **API Key**: Optional. Leave empty for Fallback Mode.

### 2. Deployment
1.  **Search**: Input Game Name -> Query Public/Private Database.
2.  **Parameters**: Toggle DLC inclusion.
3.  **EXECUTE**: DarkCore handles the rest.
    *   *Kill Steam -> Write Configs -> Inject VDF -> Restart Steam -> Trigger Install.*

---

<div align="center">
  <img src="https://img.shields.io/badge/Built_with-Love_&_Rust-red?style=plastic">
  <br>
  <sub>"Wake up, Samurai. We have a compiled language to burn."</sub>
</div>
