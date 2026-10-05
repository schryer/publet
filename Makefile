# Single entry point. Every target resolves its own tools, so nothing here
# depends on the caller's PATH, shell profile, or which machine this is.
#
# The shared targets (help, venv, lock, sync-check, fmt, fmt-check, lint,
# test, doc, build, functional) come from pubkit.mk, which pubkit manages;
# this file sets their scope and adds publet's own.
include pubkit.mk

.DEFAULT_GOAL := help
export PUBLET_BIN_DIR := $(CURDIR)/target/debug
CARGO_SCOPE := --workspace --all-features

.PHONY: bootstrap matrix coverage coverage-check conformance verify-suite verify-optional \
        guard-deps guard-floats guard-eval guard-rules publish-check deny check clean \
        release-check release-pr

bootstrap: ## Prepare a bare machine (idempotent)
	./bootstrap.sh

coverage: ## Report which normative statements have scenarios
	./tools/must-coverage.py --report=conformance/COVERAGE.md

coverage-check: coverage ## Fail if the committed coverage report is stale
	git diff --exit-code conformance/COVERAGE.md

conformance: build ## Run only the scenarios exercising a normative statement
	pytest tests/ -q -m conformance

verify-suite: ## Prove the conformance suite detects a broken build
	./conformance/verify-suite.sh

verify-optional: ## Prove the optional layers are optional
	./conformance/verify-optional.sh

guard-deps: ## Enforce the dependency direction (plan section 3)
	./tools/check-deps.py

guard-floats: ## Enforce the no-floating-point rule (R8, plan 4.1)
	./tools/check-floats.sh

guard-eval: ## Enforce that evaluation output has not changed (R8)
	./tools/check-eval-digest.sh

guard-rules: ## Enforce that every operating rule is traceable to its code
	./tools/check-rules.py

release-check: ## Check this branch records its changes, or is a correct release (BASE=origin/main)
	pubrel check $(or $(BASE),origin/main)

release-pr: ## Cut a release: publish the package publet and open a release PR
	pubrel prepare

# The crates released on their own: what each would publish, packaged and
# compiled exactly as crates.io would receive it.
PUBLISHED := publet-core publet-algorithms

publish-check: ## Package each crates.io crate and verify it builds as published
	for crate in $(PUBLISHED); do \
	  $(CARGO) publish --dry-run --locked -p $$crate; \
	done

deny: ## Licence and advisory audit
	$(CARGO) deny check

check: sync-check fmt-check lint guard-floats guard-deps guard-eval guard-rules test doc functional coverage publish-check deny ## Everything CI runs

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf .pytest_cache tests/.pytest_cache

matrix: ## Cross-architecture determinism matrix (needs podman)
	./tools/matrix.sh $(ARGS)
