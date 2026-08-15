# rune-cli

RUNE ?= rune
BINARY = target/release/rune

.PHONY: help build install validate test clean

help:
	@echo "  make build      compile the rune binary"
	@echo "  make install    install check tools, build, symlink, activate git hooks"
	@echo "  make validate   run pre-commit checks"
	@echo "  make test       validate + cargo test"
	@echo "  make clean      remove build artifacts"

build:
	cargo build --release

install:
	bash scripts/install-tools
	$(MAKE) build
	mkdir -p ~/.local/bin
	ln -sf "$(CURDIR)/$(BINARY)" ~/.local/bin/rune
	chmod +x .githooks/* scripts/install-tools scripts/install-mdschema.sh scripts/validate.sh
	git config core.hooksPath .githooks
	@if command -v jj >/dev/null 2>&1 && [ -d .jj ]; then jj config set --repo aliases.push '["util", "exec", "--", "$(CURDIR)/.githooks/jj-push"]'; fi
	@echo "Installed: rune -> $(CURDIR)/$(BINARY)"

validate:
	@bash .githooks/pre-commit --all-files

test: validate
	cargo test

clean:
	cargo clean
