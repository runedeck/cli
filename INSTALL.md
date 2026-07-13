# rune-cli

> Assemble, validate, and deploy markdown content across AI coding providers.

*This file follows the [install.md](https://github.com/mintlify/install-md) standard — structured instructions for AI agents to execute autonomously. Human readers: see [README.md](README.md) for conventional documentation.*

I want you to install rune-cli for me. Execute all the steps below autonomously.

## OBJECTIVE

Build the `rune` binary from source and make it available on PATH.

## DONE WHEN

Running `rune --help` prints the CLI usage and exits successfully.

## TODO

- [ ] Ensure Rust toolchain is installed
- [ ] Clone the repository
- [ ] Install the release binary
- [ ] Verify the installation

## Steps

### Rust toolchain

You need the Rust toolchain. If `cargo` is not available, install it:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

### Clone

```sh
git clone https://github.com/runedeck/rune.git
cd rune
```

If already cloned, pull latest:

```sh
git pull
```

### Install

```sh
cargo install --path .
```

If `~/.cargo/bin` is not on PATH:

```zsh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### Verify

You must confirm the binary is accessible:

```sh
rune --help
```

EXECUTE NOW: Complete the above TODO list to achieve: `rune --help` prints CLI usage and exits successfully.
