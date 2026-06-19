# Endur Desktop GUI

This is the native desktop application module for **Endur**, a background automatic Git backup daemon. The GUI is built on **Tauri 2.0** (Rust backend) and **Svelte 5** (TypeScript frontend), styled with a premium glassmorphic dark theme.

---

## Documentation

- **[GUI User Guide](../../docs/gui_user_guide.md)**: Detailed instructions on installing, navigating, configuring base roots, managing direct processes vs. system services, and restoring snapshots.
- **[GUI Developer Guide](../../docs/gui_developer_guide.md)**: Technical details on the architecture, Svelte 5 state management, Tauri IPC endpoints, and directory trees.

---

## Development Setup

### Prereqs
Ensure you have the Rust toolchain and Node.js installed. Refer to [Tauri Prerequisites](https://tauri.app/start/prerequisites/) for your operating system's specific C++ compiler and webview packages.

### Run in Dev Mode
1. Install node dependencies:
   ```bash
   npm install
   ```
2. Launch the Vite dev server and Tauri developer console:
   ```bash
   npm run tauri dev
   ```

### Compile Release Installer
To package the app into a native platform installer (macOS DMG, Windows MSI, or Linux DEB):
```bash
npm run tauri build
```
Production outputs will be generated under `src-tauri/target/release/bundle/`.
