# 🤝 Contributing to Vajra

First off, thank you for considering contributing to Vajra! Whether you're here to fix a bug, add a feature, or improve documentation — you're helping make Vajra better for everyone.

---

## 📋 Table of Contents

- [Code of Conduct](#-code-of-conduct)
- [Ways to Contribute](#-ways-to-contribute)
- [Reporting Bugs](#-reporting-bugs)
- [Suggesting Features](#-suggesting-features)
- [Your First Code Contribution](#-your-first-code-contribution)
- [Development Workflow](#-development-workflow)
- [Coding Standards](#-coding-standards)
- [Commit Message Format](#-commit-message-format)
- [Opening a Pull Request](#-opening-a-pull-request)

---

## 📜 Code of Conduct

This project follows a standard Code of Conduct. By participating, you agree to treat everyone with respect. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for details.

---

## 🌟 Ways to Contribute

You don't have to write code to make a meaningful contribution!

- 🐛 **Report bugs** by opening a detailed issue.
- 📖 **Improve documentation** — fix typos, add examples, or clarify instructions.
- 💡 **Suggest features** via GitHub Discussions.
- 🧪 **Test pre-releases** and report any regressions.
- 🌍 **Help with translations** (coming soon).
- 💻 **Submit code** for bug fixes or new features.

---

## 🐛 Reporting Bugs

Before reporting, please:

1. **Check existing issues** to ensure it hasn't been reported: [Issues](https://github.com/msmayanksingh22/Vajra-Download-Manager/issues)
2. **Try the latest release** to see if it's already fixed.

When you open a bug report, please include:

- **Vajra version** (found in the About dialog or `vajra --version`)
- **Operating system** and version (e.g., Windows 11 22H2, Ubuntu 24.04, macOS 14.5)
- **Steps to reproduce** the bug — be as specific as possible
- **Expected behavior** vs. **actual behavior**
- **Logs or screenshots** if applicable (the daemon log is at `~/.config/vajra/daemon.log` on Linux/macOS, or `%AppData%\Vajra\daemon.log` on Windows)

---

## 💡 Suggesting Features

Before opening a feature request, please check:
- [Open Issues](https://github.com/msmayanksingh22/Vajra-Download-Manager/issues) — it may already be planned.
- [Discussions](https://github.com/msmayanksingh22/Vajra-Download-Manager/discussions) — for broader ideas and community input.

When suggesting a feature, explain:
- **The problem** you're trying to solve (not just the solution).
- **Why** this benefits most users, not just your specific case.
- A rough sketch of how it could look or work.

---

## 🛠️ Your First Code Contribution

**Set up your development environment:**

→ See the [Developer & Contributor Guide](DEVELOPER.md) for complete per-OS setup instructions.

**Find something to work on:**

- Issues labeled [`good first issue`](https://github.com/msmayanksingh22/Vajra-Download-Manager/labels/good%20first%20issue) are great for newcomers.
- Issues labeled [`help wanted`](https://github.com/msmayanksingh22/Vajra-Download-Manager/labels/help%20wanted) are higher-priority needs.
- Feel free to ask questions in an issue before you start coding.

---

## 🔄 Development Workflow

```bash
# 1. Fork the repository on GitHub and clone your fork
git clone https://github.com/<your-username>/Vajra-Download-Manager.git
cd Vajra-Download-Manager

# 2. Add the upstream remote
git remote add upstream https://github.com/msmayanksingh22/Vajra-Download-Manager.git

# 3. Always branch off the latest main
git fetch upstream
git checkout -b feat/my-feature upstream/main

# 4. Make your changes...

# 5. Run all checks before committing (see Coding Standards below)
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# 6. Commit and push your branch
git push origin feat/my-feature

# 7. Open a Pull Request on GitHub
```

---

## 📐 Coding Standards

All contributions must pass the CI checks. Please run these locally before pushing:

### Rust

```bash
# Format all Rust code
cargo fmt --all

# Run Clippy with strict warnings (must be 0 warnings)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run tests
cargo test --workspace

# Check supply chain (licenses & security advisories)
cargo deny check
```

### TypeScript / React (Desktop UI)

```bash
cd vajra-ui-tauri
npm run lint
npx tsc --noEmit  # Type-check without building
```

### TypeScript (Browser Extension)

```bash
cd vajra-extension
npm run lint
```

### General Guidelines

- **Write tests** for any new logic you add in the Rust crates.
- **Keep changes focused** — a PR should address one thing at a time.
- **Comment non-obvious code** — especially in the engine and daemon.
- **Don't break the public API** without a strong reason and discussion.

---

## ✍️ Commit Message Format

We use [Conventional Commits](https://www.conventionalcommits.org/) for clear and automated changelogs:

```
<type>(<scope>): <short description>

[optional longer body]

[optional footer, e.g., Closes #123]
```

**Types:**

| Type | Description |
|:---|:---|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation changes only |
| `refactor` | Code restructuring (no behavior change) |
| `test` | Adding or improving tests |
| `ci` | CI/CD configuration changes |
| `chore` | Maintenance tasks (dependency bumps, tooling) |
| `perf` | Performance improvements |

**Examples:**
```bash
git commit -m "feat(engine): add HTTP/3 support via QUIC"
git commit -m "fix(cli): resolve crash when daemon is not running"
git commit -m "docs: add macOS Gatekeeper bypass instructions to README"
git commit -m "ci: upgrade Node.js to v22 in GitHub Actions"
```

---

## 🔀 Opening a Pull Request

When your branch is ready:

1. Ensure your branch is **up to date** with `upstream/main`:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```
2. Push your branch: `git push origin feat/my-feature`
3. On GitHub, click **"Compare & pull request"**.
4. Fill in the PR template:
   - **What does this PR do?** — a clear summary
   - **Why is this change needed?** — link to the related issue
   - **How was it tested?** — describe what you verified
5. A maintainer will review your PR. Please be responsive to feedback — we try to respond within a few days.

### PR Checklist

Before submitting, confirm:

- [ ] Code is formatted (`cargo fmt`, `npm run lint`)
- [ ] All tests pass (`cargo test --workspace`)
- [ ] Clippy has zero warnings (`cargo clippy -- -D warnings`)
- [ ] The commit message follows Conventional Commits format
- [ ] Documentation has been updated if needed
- [ ] If adding a new feature, tests have been added

---

Thank you for contributing to Vajra! 🎉
