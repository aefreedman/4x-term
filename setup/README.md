# Development setup

This folder contains a small, rerunnable macOS bootstrap inspired by the setup used in `nor`, but tailored to this Git- and Rust-based project. It does not install Unity or create Plastic workspaces.

## Fresh Mac

1. Install the Xcode command-line tools and [Homebrew](https://brew.sh):

   ```bash
   xcode-select --install
   ```

2. Configure GitHub SSH access, clone the repository, and enter it.

3. Run:

   ```bash
   ./setup/bootstrap-macos.sh
   ```

   Use `--skip-brew` when the Homebrew dependencies are already installed.

4. Verify the environment:

   ```bash
   ./setup/doctor.sh
   ```

5. Start `pi` from the repository root and trust the project when prompted.

## What bootstrap installs

- Git, Node.js, rustup, and the 1Password CLI through Homebrew
- The latest Pi CLI
- Machine-local `.pi/settings.json` generated from `setup/pi-packages.txt` when settings do not already exist
- The stable Rust toolchain

An existing `.pi/settings.json` is preserved so local package paths and other machine-specific choices are not overwritten. Credentials, SSH keys, Pi authentication, and 1Password authentication remain machine-local and must never be committed.
