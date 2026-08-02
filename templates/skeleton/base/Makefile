.PHONY: help install validate

help:
	@echo "  make install    activate hooks (git + jj)"
	@echo "  make validate   run commit and pre-push checks"

install:
	git config core.hooksPath .githooks
	chmod +x .githooks/* 2>/dev/null || true
	@if [ -d .jj ] && command -v jj >/dev/null 2>&1; then \
	    jj config set --repo aliases.push "[\"util\",\"exec\",\"--\",\"bash\",\"$$(git rev-parse --show-toplevel)/.githooks/jj-push\"]"; \
	    echo "jj detected: 'jj push' runs the pre-push checks, then 'jj git push'"; \
	elif [ -d .jj ]; then \
	    echo "warn: .jj/ present but jj not on PATH — 'jj push' checks NOT wired"; \
	fi

validate:
	@bash .githooks/pre-commit --all-files
	@bash .githooks/pre-push --all-files
