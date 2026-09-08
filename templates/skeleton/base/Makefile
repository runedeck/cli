.PHONY: help install validate worktree

help:
	@echo "  make install    activate hooks (git + jj)"
	@echo "  make validate   run commit-stage checks"
	@echo "  make worktree   create an agent workspace: BRANCH=<branch> IDENTITY=<model-id> HARNESS=<harness>"

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

# Create a worktree whose commits satisfy the model identity policy.
# HARNESS selects an approved domain for a new model.
worktree:
	@[ -n "$(BRANCH)" ] && [ -n "$(IDENTITY)" ] \
	    || { echo "usage: make worktree BRANCH=change/x IDENTITY=<model-id> [HARNESS=<harness>]"; exit 2; }
	@set -e; \
	identity=$$(python3 scripts/author-identity.py resolve --policy authors.yaml \
	    --model '$(IDENTITY)' --harness '$(HARNESS)'); \
	name=$$(printf '%s' "$(BRANCH)" | tr '/' '-'); \
	author=$${identity% <*}; \
	address=$${identity##*<}; address=$${address%>}; \
	if [ -d .jj ]; then \
	    mkdir -p .workspaces; \
	    jj workspace add --name "$$name" ".workspaces/$$name"; \
	    echo "jj workspace .workspaces/$$name for $(BRANCH)"; \
	    echo "author with: export JJ_USER='$$author' JJ_EMAIL='$$address'"; \
	    echo "push with the jj push alias after: jj bookmark create $(BRANCH)"; \
	else \
	    git config extensions.worktreeConfig true; \
	    git worktree add ".worktrees/$$name" -b "$(BRANCH)"; \
	    git -C ".worktrees/$$name" config --worktree user.name "$$author"; \
	    git -C ".worktrees/$$name" config --worktree user.email "$$address"; \
	    echo "worktree .worktrees/$$name on $(BRANCH) authors as $$identity"; \
	fi
