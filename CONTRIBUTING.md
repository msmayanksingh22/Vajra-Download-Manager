# Contributing to Vajra

First off, thank you for considering contributing to Vajra! It's people like you that make Vajra such a great tool.

## Where do I go from here?

If you've noticed a bug or have a feature request, make one! It's generally best if you get confirmation of your bug or approval for your feature request this way before starting to code.

## Fork & create a branch

If this is something you think you can fix, then fork Vajra and create a branch with a descriptive name.

## Get the test suite running & Build the Project

To compile the backend, frontend, or browser extension, please refer to our comprehensive [Developer & Contributor Guide](DEVELOPER.md). It contains all the prerequisites, build scripts, and manual compilation steps.

## Implement your fix or feature

At this point, you're ready to make your changes. Feel free to ask for help; everyone is a beginner at first.

## Code Quality

Before pushing, please run:
```bash
cargo fmt
cargo clippy -- -D warnings
cargo deny check
```

For frontend changes, run:
```bash
cd vajra-ui-tauri
npm run lint
npm run format:check
```

## Make a Pull Request

At this point, you should switch back to your master branch and make sure it's up to date with Vajra's master branch:

```bash
git remote add upstream https://github.com/msmayanksingh22/Vajra-Download-Manager.git
git checkout main
git pull upstream main
```

Then update your feature branch from your local copy of master, and push it!
