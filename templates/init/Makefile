RUNE ?= rune

.PHONY: help install validate clean

help:
	@echo "  make install    deploy and activate git hooks"
	@echo "  make validate   validate module structure and code"
	@echo "  make clean      remove build artifacts"

install:
	@command -v $(RUNE) >/dev/null 2>&1 \
	    || { echo "rune not found — ask an AI assistant to execute INSTALL.md"; exit 1; }
	git config core.hooksPath .githooks
	$(RUNE) install

validate:
	@bash .githooks/pre-commit

clean:
	rm -rf build/
