# rune-cli

RUNE ?= rune
BINARY = target/release/rune

.PHONY: help build install validate test clean

help:
	@echo "  make build      compile the rune binary"
	@echo "  make install    build, symlink, activate git hooks"
	@echo "  make validate   run pre-commit checks"
	@echo "  make test       validate + cargo test"
	@echo "  make clean      remove build artifacts"

build:
	cargo build --release

install: build
	mkdir -p ~/.local/bin
	ln -sf "$(CURDIR)/$(BINARY)" ~/.local/bin/rune
	git config core.hooksPath .githooks
	@echo "Installed: rune -> $(CURDIR)/$(BINARY)"

validate:
	@bash .githooks/pre-commit

test: validate
	cargo test

clean:
	cargo clean
