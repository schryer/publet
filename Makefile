# Single entry point. Every target resolves its own tools, so nothing here
# depends on the caller's PATH, shell profile, or which machine this is.
SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

export PATH := $(HOME)/.cargo/bin:$(CURDIR)/.venv/bin:$(PATH)
export PUBLET_BIN_DIR := $(CURDIR)/target/debug
CARGO := cargo

.PHONY: help bootstrap lock fmt fmt-check lint test doc functional matrix \
        guard-deps guard-floats deny check build clean

help: ## Show available targets
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk -F':.*?## ' '{printf "  \033[1m%-14s\033[0m %s\n", $$1, $$2}'

bootstrap: ## Prepare a bare machine (idempotent)
	./bootstrap.sh

lock: ## Regenerate tests/requirements.lock from requirements.txt
	./tools/lock-python.sh

build: ## Build the workspace
	$(CARGO) build --workspace --all-targets

fmt: ## Format Rust sources
	$(CARGO) fmt --all

fmt-check: ## Verify formatting without modifying
	$(CARGO) fmt --all -- --check

lint: ## Clippy across the workspace, warnings are errors
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

test: ## Rust unit, integration, and doc tests
	$(CARGO) test --workspace --all-features

doc: ## Build documentation, warnings are errors
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps

functional: build ## Python/Gherkin functional suite against built binaries
	pytest tests/ -q

guard-deps: ## Enforce the dependency direction (plan section 3)
	./tools/check-deps.py

guard-floats: ## Enforce the no-floating-point rule (R8, plan 4.1)
	./tools/check-floats.sh

deny: ## Licence and advisory audit
	$(CARGO) deny check

check: fmt-check lint guard-floats guard-deps test doc functional ## Everything CI runs

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf .pytest_cache tests/.pytest_cache

matrix: ## Cross-architecture determinism matrix (needs podman)
	./tools/matrix.sh $(ARGS)
