# DMMGamePlayer-Wine

[![Check & Build](https://github.com/Tenshou170/DMMGamePlayer-Wine/actions/workflows/update.yml/badge.svg)](https://github.com/Tenshou170/DMMGamePlayer-Wine/actions/workflows/update.yml)
[![Release](https://img.shields.io/github/v/release/Tenshou170/DMMGamePlayer-Wine)](https://github.com/Tenshou170/DMMGamePlayer-Wine/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Wine and Proton compatibility repack and native setup wizard for the official DMM Game Player (v5.x) client.

This repository tracks upstream releases, extracts the application payload from the NSIS installer to bypass PowerShell and UAC execution failures under Wine, and provides a standalone Win32 setup wizard (`DMMGamePlayer-Setup-Wine.exe`) that executes directly inside Wine and Proton prefixes to inspect environment health, install the application from an embedded payload, and configure Windows registry settings.

---

## Overview

Modern versions of the official DMM Game Player installer (`DMMGamePlayer-Setup-*.exe`) fail under Wine and Proton due to installer-level dependencies on Windows subsystems:
1. **PowerShell CIM Invocation:** The installer invokes `powershell.exe -C "Get-CimInstance -ClassName Win32_Process ..."` to inspect and terminate running processes. Wine's minimal PowerShell implementation does not support CIM cmdlets, resulting in an indefinite process hang.
2. **UAC Elevation Hooks:** Dual-process elevation routines (`UAC.dll` and `SpiderBanner.dll`) fail under Wine's security token and process isolation model.
3. **Registry Initialization:** The local gRPC daemon settings (port `14603`) and protocol handlers (`dmmgameplayer://`) required for game authentication and session management are not registered when the setup executable fails.

### Native Setup Wizard Architecture

Rather than relying on fragile external host scripts that attempt to parse prefix directories and runner binaries from the outside, this project provides a standalone PE setup package (`DMMGamePlayer-Setup-Wine.exe`) built in Rust targeting `x86_64-pc-windows-msvc`.

* **All-in-One Standalone Packaging:** The released setup executable embeds the compressed application payload directly as a PE overlay (~127 MB, matching upstream installer size). It extracts files in-memory directly to disk via pure-Rust LZMA2 decompression (`sevenz-rust2`) without requiring external side-by-side archives or folders.
* **Refined Win32 GUI Wizard:**
  - Standard system font styling (`DS_SHELLFONT` / `MS Shell Dlg`), allowing Wine/Proton and Linux font aliasing to provide clean CJK font rendering without proprietary font hardcoding.
  - Balanced 3-action buttons: **Install / Update**, **Repair Registry**, and **Uninstall**.
  - Integrated system menu and titlebar close button (`X`).
  - Real-time diagnostic evaluation of the Wine prefix, active runner, and prerequisite state.
* **Native Registry Management:** Applies registry configurations directly to `HKLM` and `HKCU` via official Microsoft registry bindings (`windows-registry`), configuring SDK port `14603`, the `dmmgameplayer://` URL protocol handler, and Windows Add/Remove Programs uninstall entries.
* **Headless CLI Interface:** Exposes subcommands and silent flags (`/S`, `--silent`) for scriptable, automated prefix provisioning.

---

## Technical Caveats & Runtime Behavior

### 1. Game Launch & Authentication Architecture
* **Mechanism:** DMM titles require ephemeral session tokens generated at launch time. The client passes these authentication arguments directly to the game binary upon execution and exposes a local gRPC authentication service on `127.0.0.1:14603` (hosted by `windows_amd64_cex.dll`).
* **Requirement:** Games **must** be launched directly through the DMM Game Player interface or via the registered `dmmgameplayer://` protocol handler. Manually executing the standalone game `.exe` outside the launcher context will result in authentication failure or immediate termination due to missing launch parameters.

### 2. GAMES Play Assist (Error Code 224003)
* **Behavior:** A notification may appear stating `GAMESプレイアシストの起動に失敗しました。ゲームの起動には影響ありません。(エラーコード: 224003)`.
* **Technical Cause:** `GamesPlayAssist.exe` is an auxiliary background process written in Rust that provides in-game overlays and capture tools. It requires the Microsoft Edge WebView2 runtime and Windows desktop capture hooks.
* **Status:** Non-fatal. Client authentication and game execution proceed normally regardless of this notification.
* **Resolution:** Installing `webview2` via winetricks or your prefix manager satisfies the runtime dependency and suppresses the warning.

### 3. Font Dependencies (CJK Rendering)
* **Requirement:** Unity and native Japanese game engines will terminate immediately on startup if Japanese font glyphs (Meiryo / MS Gothic) are missing from the prefix.
* **Resolution:** Install `cjkfonts` into the prefix.

### 4. Locale Configuration
* **Requirement:** Japanese locale settings must be defined to prevent character encoding errors, file path corruptions, and runtime exceptions.
* **Configuration:** Set `LANG=ja_JP.UTF-8` and `LC_ALL=ja_JP.UTF-8` in the execution environment.

### 5. Media Foundation & Video Playback
* **Requirement:** In-game cutscenes, opening animations, and video streams utilizing Criware Sofdec2 or VP9/H.264 require Media Foundation and GStreamer codecs.
* **Recommended Runners:** GE-Proton, Wine-GE, Soda, or Caffe.

### 6. Upstream In-App Auto-Updates
* **Notice:** Do not use the in-app client update prompt. The updater downloads the official NSIS installer, re-triggering the installer execution failure under Wine. Update by downloading the latest `DMMGamePlayer-Setup-Wine-<VERSION>.exe` release and running **Install / Update** (or `DMMGamePlayer-Setup-Wine.exe install`).

---

## Setup Wizard Usage

### GUI Installation
Download `DMMGamePlayer-Setup-Wine-<VERSION>.exe` (or extract `DMMGamePlayer-v<VERSION>-portable.zip`). Execute `DMMGamePlayer-Setup-Wine.exe` within the target prefix:

* **Bottles:**
  1. Open target bottle -> Click **Run Executable**.
  2. Select `DMMGamePlayer-Setup-Wine-<VERSION>.exe`.
  3. Verify environment diagnostics in the wizard window and click **Install / Update**.

* **Lutris:**
  1. Select the configured game or bottle.
  2. Click the Wine dropdown menu next to the Play button -> **Run EXE inside Wine prefix**.
  3. Select `DMMGamePlayer-Setup-Wine-<VERSION>.exe` and follow the on-screen prompts.

* **Terminal / Standalone Wine:**
  ```bash
  WINEPREFIX="/path/to/prefix" wine DMMGamePlayer-Setup-Wine.exe
  ```

### Command-Line Reference (Headless Mode)

The installer binary supports non-interactive execution for automated setups:

```bash
# Print environment diagnostics (runner version, font check, webview2, registry state)
wine DMMGamePlayer-Setup-Wine.exe diagnostics

# Install payload and configure registry non-interactively
wine DMMGamePlayer-Setup-Wine.exe install --silent

# Install to a custom directory
wine DMMGamePlayer-Setup-Wine.exe install --install-dir "C:\Games\DMMGamePlayer" --silent

# Re-apply SDK port and protocol handler keys without modifying files
wine DMMGamePlayer-Setup-Wine.exe repair

# Remove installation and delete registry keys
wine DMMGamePlayer-Setup-Wine.exe uninstall --silent
```

#### CLI Subcommands & Options

| Command / Flag | Argument | Description |
| :--- | :--- | :--- |
| `install` | — | Extracts payload to target directory and registers all registry settings |
| `repair`, `repair-reg` | — | Reconfigures SDK gRPC port (`14603`), protocol scheme, and directories |
| `uninstall` | — | Removes installation directory and deletes DMM registry keys |
| `diagnostics` | — | Evaluates runner version, CJK fonts, WebView2, and SDK registration |
| `--install-dir` | `<PATH>` | Sets destination directory (default: `C:\Program Files\DMMGamePlayer`) |
| `--silent`, `/S` | — | Suppresses interactive dialogs and message boxes |
| `-h`, `--help` | — | Displays syntax reference |

---

## Prefix Configuration Reference

### Recommended Prefix Packages (Winetricks / Bottles / Lutris)

| Component | Identifier | Purpose |
| :--- | :--- | :--- |
| Japanese Fonts | `cjkfonts` | Prevents text rendering crashes in Unity and native game engines |
| Edge WebView2 | `webview2` | Suppresses GamesPlayAssist error 224003; enables embedded web views |
| VC++ Runtime | `vcredist2019` | Standard runtime dependency for game client modules |
| HLSL Compiler | `d3dcompiler_47` | DirectX HLSL shader compilation |

### Recommended Environment Variables

```ini
LANG=ja_JP.UTF-8
LC_ALL=ja_JP.UTF-8
```

---

## Building from Source

### Prerequisites
* Rust toolchain (stable)
* `x86_64-pc-windows-msvc` target (`rustup target add x86_64-pc-windows-msvc`)
* `cargo-xwin` (`cargo install --locked cargo-xwin`)
* `p7zip`, `python3`, `curl`, `jq`

### Building the Setup Wizard Binary
```bash
cargo xwin build --release --target x86_64-pc-windows-msvc
# Output: target/x86_64-pc-windows-msvc/release/DMMGamePlayer-Setup-Wine.exe
```

### Full Repack Pipeline
```bash
# Build latest upstream release
./scripts/build.sh

# Build a specific upstream version
./scripts/build.sh 5.5.19
```

The script extracts the official NSIS payload, builds `DMMGamePlayer-Setup-Wine.exe` using `cargo-xwin`, embeds the compressed payload overlay directly into the standalone installer binary, generates registry definitions, and outputs release archives into `dist/v<VERSION>/`.

---

## Automated CI & Verification Pipeline

This repository runs an automated GitHub Actions workflow every week (`.github/workflows/update.yml`):
1. **Upstream Feed Query:** Queries `https://dlapp-dmmgameplayer.games.dmm.com/latest.yml` for new releases.
2. **Tag Comparison:** Compares detected version against existing repository tags. If already published, skips cleanly.
3. **Dataset Verification:** Unpacks the installer and queries all discovered registry keys, gRPC authentication parameters, and binary files against the verified baseline (`data/known_dataset.json`).
   - If an unexpected new registry key, altered port/GUID, missing crucial file, or new binary addition is detected, the workflow immediately halts and writes a detailed divergence report with required manual intervention steps to the GitHub Actions Job Summary (`$GITHUB_STEP_SUMMARY`).
4. **Version Bump & Build:** Updates `Cargo.toml`, compiles `DMMGamePlayer-Setup-Wine.exe` with `cargo-xwin`, packages portable archives, publishes a new GitHub release, and commits the bumped version to source code (`[skip ci]`).

### Upstream References
* **Official DMM Game Player Changelog & Notices:** https://support.dmm.com/games/subcategory/770
* **Official DMM Game Player Portal:** https://player.games.dmm.com/
* **Upstream Version Manifest:** https://dlapp-dmmgameplayer.games.dmm.com/latest.yml

---

## License

The installer source code and scripts in this repository are licensed under the [MIT License](LICENSE).  
DMM Game Player binaries and related trademarks are the property of EXNOA LLC / DMM.com.
