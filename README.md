# Secretlink

[![CI](https://github.com/coder3101/secretlink/actions/workflows/ci.yml/badge.svg)](https://github.com/coder3101/secretlink/actions/workflows/ci.yml)

**Secretlink** is a blazing-fast, open-source, one-time secret sharing application built with Rust and PostgreSQL. Share sensitive information—like passwords, API keys, or confidential notes—securely and privately. Each link is end-to-end encrypted in your browser and can only be viewed once before it self-destructs.

---

## 🚀 Features

- **End-to-End Encryption:** Secrets are encrypted in your browser before ever reaching the server.
- **One-Time Access:** Each link can only be viewed once. After that, it’s gone forever.
- **No Registration:** Share secrets instantly—no account or login required.
- **Flexible Expiry:** Set secrets to expire after a certain time or keep them available until first view.
- **Open Source:** MIT licensed and easy to self-host.
- **Modern Rust Stack:** Built with [Axum](https://github.com/tokio-rs/axum), [SQLx](https://github.com/launchbadge/sqlx), and [Askama](https://github.com/askama-rs/askama).

---

## 🏗️ How It Works

1. **Write your secret** in the web UI.
2. **Choose an expiry** (one-time, 1 hour, 24 hours, or 1 week).
3. **Generate a secure link**—the secret is encrypted in your browser.
4. **Share the link**. The recipient can view the secret once; after that, it’s deleted.

---

## 📦 Getting Started

### Prerequisites

- Rust (latest stable)
- PostgreSQL

### Running Locally

1. **Clone the repo:**
   ```sh
   git clone https://github.com/coder3101/secretlink.git
   cd secretlink
   ```

2. **Set up the database:**
   ```sh
   # After your postgresql is up and running, set the following environment
   export DATABASE_URL=postgres://postgres:password@localhost:5432/secretlink
   ```

3. **Run migrations:**
   ```sh
   cargo install sqlx-cli --no-default-features --features postgres
   sqlx migrate run
   ```

4. **Start the server:**
   ```sh
   cargo run
   ```

5. Visit [http://localhost:8080](http://localhost:8080)

---

## 🧪 Running Tests

```sh
export DATABASE_URL=postgres://postgres:password@localhost:5432/secretlink
cargo test
```

---

## 🛡️ Security

- Secrets are encrypted in the browser using AES-256-GCM.
- The server never sees the plaintext secret or the encryption key.
- Links are single-use and self-destruct after being viewed.

---

## 🤝 Contributing

Contributions are welcome! Please open issues or pull requests.

---

## 📄 License

MIT License © 2025 Ashar

---

## 🙏 Acknowledgements

- [Axum](https://github.com/tokio-rs/axum)
- [SQLx](https://github.com/launchbadge/sqlx)
- [Askama](https://github.com/askama-rs/askama)

---

> Made with ❤️ in Rust
