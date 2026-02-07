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


---

## ✨ Feature Matrix

### 🟢 Hybrid Operation Mode
DarkCore adapts to your environment in real-time.

| Mode              | Status          | Functionality                                                                                               |
| :---------------- | :-------------- | :---------------------------------------------------------------------------------------------------------- |
| **Authenticated** | **FULL ACCESS** | Complete API integration. Precise AppID resolution. Manifest/Lua synchronization enabled.                   |
| **Standard**      | **LOCAL ONLY**  | **Fallback to Public Store API**. Instant Search & DLC listings. Ideal for local Family Sharing management. |

> [!NOTE]
> **BYOL (Bring Your Own License)**: GreenLuma 1.7.0 (not 1.7.1), Steamless, and Morrenus API Keys are third-party resources. You must acquire and configure them independently.

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
* **Native Downloader**: Download using Steam or using our external Rust DepotDownloader! you choose! 

---

## 🛠️ Compilation Source

We believe in transparency. Build it yourself.

### Prerequisites
* [**Rust Toolchain (rustup)**](https://rustup.rs/)
* **Git** & **Windows SDK**

### Build Sequence

1.  **Clone Repository**
    ```powershell
    git clone https://github.com/hkmodd/DarkCore-Manager.git
    cd DarkCore-Manager
    ```
2.  **Compile System**
    ```powershell
    cargo build --release
    ```
    *Output:*
    * **Manager**: `target/release/darkcore-manager.exe` (The UI Application)

---

## ⚙️ Operational Protocol

### 1. Environment Mapping (Initialization)
Upon first boot, the core system requires mapping to your local ecosystem. Navigate to the **SETTINGS** tab to initialize the environment:

* **Steam Root**: The directory housing the client executable.
* **Compatibility Artifact**: The folder containing the *GreenLuma* binary (`GreenLuma_2025_x64.dll`).
* **Preprocessor Binary**: Path to `Steamless.CLI.exe`.
* **API Key**: (Optional) Input your private API key for metadata retrieval.

### 2. The Execution Cycle
DarkCore streamlines the deployment process into a deterministic linear workflow:

1.  **Query**: Navigate to **SEARCH**, input a specific AppID or Game Name.
2.  **Engage**: Click **INSTALL** for metadata integration, Right-click for basic AppID listing.
3.  **Steamless (Optional)**: For offline play, use the **STEAMLESS** button in the Library to patch DRM-protected executables.

> [!TIP]
> **Process Hygiene**: DarkCore handles process termination during critical operations (Injection) to prevent file locking.

---

<div align="center">
  <img src="https://img.shields.io/badge/Built_with-Love_&_Rust-red?style=plastic">
  <br>
  <sub>"Wake up, Samurai. We have a compiled language to burn."</sub>
</div>
