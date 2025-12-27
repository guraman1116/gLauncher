# gLauncher 🚀

**gLauncher** is a lightweight, customizable, and high-performance Minecraft Java Edition launcher written in **Rust**.

Designed for speed and simplicity, it supports modern mod loaders like **Fabric** and **Forge** out of the box, with both a sleek GUI and a powerful CLI.


## ✨ Key Features

- **🚀 Blazing Fast**: Built with Rust for minimal resource usage and instant startup.
- **🛠️ Mod Loader Support**: Seamless integration for **Vanilla**, **Fabric**, and **Forge**.
- **🔑 Microsoft Authentication**: Secure login flow with device code authentication.
- **📦 Instance Management**: Create, isolate, and manage multiple Minecraft instances easily.
- **🖥️ Dual Interface**:
  - **GUI**: User-friendly `egui`-based interface for everyday use.
  - **CLI**: Fully functional command-line interface for automation and power users.
- **🔄 Auto-Update**: Built-in self-update mechanism via GitHub Releases.
- **🌍 Cross-Platform**: Runs on **macOS**, **Windows**, and **Linux**.(You should build manually for now)

## 📥 Installation

### Download Binary
Visit the [Releases Page](https://github.com/guraman1116/gLauncher/releases) and download the installer for your operating system.

- **macOS**: `.dmg` or `.app` bundle
- **Windows**: `.exe` installer
- **Linux**: Binary executable

### Build from Source
Ensure you have the latest [Rust toolchain](https://rustup.rs/) installed.

```bash
git clone https://github.com/guraman1116/gLauncher.git
cd gLauncher
cargo build --release
```

The executable will be located in `target/release/glauncher`.

## 🎮 Usage

### GUI Mode
Simply run the application to open the graphical interface.

```bash
./glauncher
```

- **Login**: Click "Login with Microsoft" and follow the device code instructions.
- **Create Instance**: Click "➕ New", select Version and Mod Loader (Fabric/Forge).
- **Launch**: Click "▶️ Launch" on any instance.

### CLI Mode
gLauncher provides a powerful CLI for headless operation.

**List Instances:**
```bash
./glauncher --list
```

**Launch an Instance:**
```bash
./glauncher -i "MyFabricInstance"
```
*(Add `--offline` for offline mode if previously authenticated)*

**Create an Instance:**
```bash
./glauncher create --name "MyForge" --version "1.20.1" --loader forge
```

## 🛠️ Development

### Prerequisites
- Rust (latest stable)
- `cmake` (for building dependencies)

### Running Locally
```bash
cargo run
```

### Building Installers
Install `cargo-bundle`:
```bash
cargo install cargo-bundle
cargo bundle --release
```

## 🤝 Contributing
Contributions are welcome! Please open an issue or submit a pull request.

## 📄 License
This project is licensed under the [MIT License](LICENSE).

---
© 2025 guraman
