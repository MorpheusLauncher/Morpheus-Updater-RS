<div align="center">

# Morpheus Updater

**Native Rust installer and updater for Morpheus Launcher.**

![Language](https://img.shields.io/badge/language-Rust-CE422B?logo=rust\&logoColor=white)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-4B5563)
![Engineering Confidence: 5/10](https://img.shields.io/badge/Engineering%20Confidence-5%2F10-F9A825)

</div>

---

## 📦 About

**Morpheus Updater** is the native installer and update manager for Morpheus Launcher.

Written in Rust, it provides a lightweight graphical bootstrap process for Windows and Linux that can install the launcher, check for updates and start the local installation automatically.

Its main goal is to keep the Morpheus Launcher installation process as simple as possible:

```text
Start Morpheus Updater
        ↓
Check local installation
        ↓
Check remote version
        ↓
┌──────────────────────────┐
│ Up to date               │ → Launch Morpheus
│ Update available         │ → Download update
│ First installation       │ → Install everything
│ Offline / server error   │ → Launch local version
└──────────────────────────┘
```

## ⚡ Features

* ✅ **Native Rust application**
* ✅ **Windows support**
* ✅ **Linux support**
* ✅ **Automatic first-time installation**
* ✅ **Automatic update checking**
* ✅ **Offline fallback to the installed launcher**
* ✅ **Parallel file downloads**
* ✅ **Temporary `.part` files during downloads**
* ✅ **Automatic ZIP extraction**
* ✅ **Automatic launcher startup**
* ✅ **Self-relocation into the Morpheus installation directory**
* ✅ **Automatic shortcut / application entry creation**
* ✅ **Basic Linux dependency management**
* ✅ **Simple graphical installation log**

## 🔄 Update process

When Morpheus Updater starts, it checks whether a valid local installation and version file already exist.

If they do, the updater contacts the Morpheus server and compares the installed version against the latest available version.

### Up to date

If both versions match, Morpheus Launcher is started immediately.

### Update available

If the remote version is newer, the required files are downloaded and installed before starting the launcher.

### First installation

If no valid local installation exists, the updater automatically performs the initial installation.

### Offline mode

If the update server cannot be reached but a local installation is already available, the updater skips the update and starts the installed launcher instead.

This prevents a temporary network or server problem from making an existing Morpheus installation unusable.

## 📥 Installed files

The updater downloads the platform-specific Morpheus package together with the launcher components required by the installation.

Depending on the operating system, the native package is selected automatically.

Downloaded archives are extracted directly into the Morpheus installation directory.

Files are initially written using a temporary `.part` extension and renamed after the download has completed.

## 📁 Installation directory

Morpheus Updater keeps the launcher installation inside the user's home environment.

### Windows

```text
%APPDATA%\.morpheus
```

### Linux

```text
~/.morpheus
```

The updater also relocates itself into this directory so subsequent launches use the installed copy.

## 🖥️ Desktop integration

### Windows

The updater creates a **Morpheus Launcher** shortcut on the user's desktop.

### Linux

A `.desktop` application entry is created under:

```text
~/.local/share/applications/
```

This allows Morpheus Launcher to appear in compatible desktop application menus.

## 🐧 Linux dependency handling

On Linux, Morpheus Updater performs basic checks for native libraries required by the launcher.

Supported package-manager families include:

* Debian / Ubuntu
* Fedora / RHEL
* Arch / Manjaro

When dependencies are missing, the updater can request elevated privileges through `pkexec` and invoke the appropriate package manager.

It also contains compatibility handling for different `libjsoncpp` versions when required by the launcher runtime.

> [!IMPORTANT]
> Automatic dependency handling is best-effort.
>
> Unsupported distributions or unusual system configurations may require manual intervention.

## 🪟 Windows behavior

On Windows, Morpheus Launcher is started independently from the updater so the installer can close without terminating the launcher.

The updater runs without opening an additional console window.

## 🧠 Engineering confidence

**5/10 — Thoroughly tested AI-written utility**

Morpheus Updater was implemented entirely with AI assistance.

The application has been extensively tested and refined across its intended installation and update workflow, and it is considered stable for normal use.

However, an updater interacts with the filesystem, network, operating-system APIs, package managers and external launcher components. Because of this, unexpected environments or edge cases can still produce bugs or incorrect behavior even when the normal workflow has been thoroughly validated.

The score therefore reflects the **engineering process, code ownership, platform complexity and remaining uncertainty around uncommon failure cases**.

It does not mean that the updater is known to be unreliable; it means that the project does not claim complete absence of bugs or platform-specific malfunctions.

## ⚠️ Reliability and limitations

Morpheus Updater is designed to fail gracefully where possible, but no installer or updater can guarantee compatibility with every system configuration.

Potential issues can include:

* interrupted or corrupted downloads;
* filesystem permission problems;
* antivirus or security software interference;
* unsupported Linux distributions;
* missing system utilities;
* unusual home-directory configurations;
* package-manager failures;
* incompatible native libraries;
* desktop environments that do not use standard `.desktop` entries;
* server-side availability problems.

Existing installations can still be launched when the remote version check fails, provided the local launcher installation remains valid.

## 🔐 Privileges

The updater itself does not normally need to run permanently as administrator or root.

On Linux, elevated privileges can be requested through `pkexec` when system packages or compatibility symlinks need to be installed.

Users should review privilege prompts before accepting them.

## 🛠️ Built with

Morpheus Updater uses Rust together with:

* **egui / eframe** — native graphical interface;
* **reqwest** — HTTP requests;
* **Tokio** — asynchronous runtime and downloads;
* **zip** — launcher archive extraction;
* **image** — embedded application icon handling.

## 🔗 Morpheus Launcher

Morpheus Updater exists as part of the wider **Morpheus Launcher** ecosystem.

Its responsibility is limited to installation, updating and starting the launcher.

The Minecraft launch logic itself remains in Morpheus Launcher and its graphical frontend.

## 🧪 Project status

The updater has been tested extensively for its intended workflow, but should still be treated as software that can encounter unforeseen system-specific bugs.

If an update fails unexpectedly, keep a copy of important local data and report the environment, operating system and error shown by the updater.

## Warranty

> [!CAUTION]
> This software is provided **as-is**, without warranty of any kind.
>
> Although the updater has been extensively tested, complete absence of bugs, installation failures or platform-specific problems is not guaranteed.

---

<div align="center">

**Install. Update. Launch.**

</div>
