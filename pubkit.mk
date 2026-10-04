# Managed by pubkit 0.3.0: `pubkit sync` rewrites this file, and
# `pubkit sync --check` fails when it differs. Include it from the Makefile,
# and override the variables below there; targets of your own go there too.
SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
export PATH := $(HOME)/.cargo/bin:$(CURDIR)/.venv/bin:$(PATH)

CARGO ?= cargo
CARGO_SCOPE ?= --workspace
PYTEST_ARGS ?=

.PHONY: help venv lock sync-check fmt fmt-check lint test doc build functional

help: ## Show available targets
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk -F':.*?## ' '{printf "  %-14s %s\n", $$1, $$2}'

venv:: ## Python environment for the functional suite, hash-locked
	python3 -m venv .venv
	.venv/bin/pip install --quiet --require-hashes -r tests/requirements.lock

lock: ## Pin tests/requirements.txt, with hashes, in tests/requirements.lock
	.venv/bin/pubkit lock

sync-check: ## Fail if pubkit's managed files differ from the pinned version's
	.venv/bin/pubkit sync --check

fmt: ## Format Rust sources
	$(CARGO) fmt --all

fmt-check: ## Verify formatting without modifying
	$(CARGO) fmt --all -- --check

lint: ## Clippy, warnings are errors
	$(CARGO) clippy $(CARGO_SCOPE) --all-targets -- -D warnings

test: ## Rust unit tests
	$(CARGO) test $(CARGO_SCOPE)

doc: ## Documentation, warnings are errors
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc $(CARGO_SCOPE) --no-deps

build: ## Debug build
	$(CARGO) build $(CARGO_SCOPE)

functional: build ## The Gherkin suite against the built binaries
	cd tests && pytest -q $(PYTEST_ARGS)
