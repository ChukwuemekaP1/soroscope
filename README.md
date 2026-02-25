# 🔬 SoroScope: Soroban Resource Profiler

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Stellar Wave](https://img.shields.io/badge/Stellar-Wave_Program-blue)](https://www.drips.network/wave/stellar)

**SoroScope** is a developer tool designed to provide deep visibility into Soroban smart contract resource consumption (CPU, RAM, and Ledger Footprint).

## 🚀 The Vision
Building on Soroban requires careful resource management. SoroScope provides a "Nutrition Label" for your smart contracts, helping you optimize for lower fees and higher performance before you deploy to Mainnet.

## 🧱 Monorepo Structure
- `/core`: Rust-based CLI for simulating and profiling contracts.
- `/web`: Next.js + Tailwind CSS dashboard for visualizing resource heatmaps.
- `/contracts`: Sample Soroban contracts used for benchmarking.
- `/.github/workflows`: CI/CD pipelines.

## ⚙️ Getting Started

### Prerequisites
- **Rust** (stable, via [rustup](https://rustup.rs))
- **Node.js** (>= 18) and **npm** / **pnpm** / **yarn**
- Soroban CLI & tooling (recommended) for real-network interaction

### Clone the Repository
```bash
git clone https://github.com/SoroLabs/soroscope
cd soroscope
```

---

## 🧰 Core CLI (`/core`)

The **core** crate is a Rust binary that will power SoroScope's resource profiling.

### Build & Run
```bash
# Build the binary
cargo build -p soroscope-core

# Run the server (RUST_LOG=info is required to see API logs)
RUST_LOG=info cargo run -p soroscope-core
```

The server listens on `http://localhost:8080` by default.

---

## 🌐 Web Dashboard (`/web`)

The **web** app is a Next.js + Tailwind CSS dashboard for exploring resource usage visually.

### Install Dependencies
```bash
cd web
npm install        # or: pnpm install / yarn install
```

### Run in Development
```bash
npm run dev
```

Then open:
- http://localhost:3000

### Build for Production
```bash
npm run build
npm start
```

---

## 📦 Contracts (`/contracts`)

This folder contains sample Soroban contracts. To build them for analysis:

```bash
# Build all contracts to WASM
cargo build --target wasm32-unknown-unknown --release
```

The resulting `.wasm` files will be located in `target/wasm32-unknown-unknown/release/`. You can upload these to the Web Dashboard for profiling.

---



## 📅 Roadmap (2026)
- **Phase 1 [COMPLETED]:** Core CLI engine for resource extraction.
- **Phase 2 [IN PROGRESS]:** Integration of Frontend dashboard with Backend simulation engine.
- **Phase 3:** Automated optimization recommendations.

---

## 🧪 Development & Scripts

From the **repo root**:

- Check workspace builds:
  ```bash
  cargo build
  ```

- Format Rust code:
  ```bash
  cargo fmt
  ```

- Lint / type-check web app:
  ```bash
  cd web
  npm run lint
  ```

(Add CI in `./.github/workflows` to automate these.)

---

## 🤝 Contributing
Contributions are welcome! Please read our [**Contributing Guide**](./CONTRIBUTING.md) to learn about our development process, coding standards, and how to submit a pull request.

---
### 🧪 Live Analysis
SoroScope now supports live simulation via the web dashboard. Connect your wallet, select a function, and get your **Contract Nutrition Label** instantly.

---
Built with ❤️ by **SoroLabs**. Powered by the Soroban ecosystem.