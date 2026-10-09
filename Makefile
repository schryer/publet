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
        guard-deps guard-floats guard-eval guard-rules publish-check msrv fuzz-smoke vectors vectors-check doc-coverage doc-coverage-check supply-chain supply-chain-check vet deny check clean \
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

# The verify builds get a target directory of their own. Sharing target/
# leaves dep-info naming the packaged copies' sources, after which cargo
# can report a crate Fresh however its src/ changes.
publish-check: ## Package each crates.io crate and verify it builds as published
	for crate in $(PUBLISHED); do \
	  CARGO_TARGET_DIR=$(CURDIR)/target/publish-check $(CARGO) publish --dry-run --locked -p $$crate; \
	done

# The published crates' minimum Rust version, as their manifests declare it.
MSRV := $(shell sed -n 's/^rust-version = "\(.*\)"/\1/p' crates/publet-core/Cargo.toml)

msrv: ## Build the published crates on their minimum Rust version (rustup toolchain install $(MSRV))
	@test "$(MSRV)" = "$$(sed -n 's/^rust-version = "\(.*\)"/\1/p' crates/publet-algorithms/Cargo.toml)" \
	  || { echo "publet-core and publet-algorithms declare different minimum Rust versions"; exit 1; }
	$(CARGO) +$(MSRV) check --locked -p publet-core -p publet-algorithms

FUZZ_TARGETS := cbor_decode object_parse cid_parse log_verify signature_verify
FUZZ_SECONDS ?= 60

fuzz-smoke: ## Replay fuzz regressions, then fuzz each target briefly (needs nightly and cargo-fuzz)
	cd fuzz && cargo +nightly fuzz build
	cd fuzz && for target in $(FUZZ_TARGETS); do \
	  mkdir -p corpus/$$target regressions/$$target; \
	  cargo +nightly fuzz run $$target corpus/$$target regressions/$$target -- \
	    -max_total_time=$(FUZZ_SECONDS) -timeout=10 || exit 1; \
	done

vectors: ## Regenerate the published crates' test vectors from independent sources
	./tools/vectors/generate.py

vectors-check: vectors ## Fail if the committed test vectors differ from what the sources give
	git diff --exit-code -- crates/publet-core/testdata crates/publet-algorithms/testdata

doc-coverage: ## Regenerate each published crate's TESTING.md
	./tools/doc-coverage.py

doc-coverage-check: ## Fail if a public function has no example, a test covers no real item, or TESTING.md is stale
	./tools/doc-coverage.py --check

supply-chain: ## Regenerate each published crate's SUPPLY-CHAIN.md from supply-chain/
	./tools/supply-chain-report.py

supply-chain-check: ## Fail if a committed SUPPLY-CHAIN.md is stale
	./tools/supply-chain-report.py --check

vet: ## Every dependency is audited, trusted, or exempt with its evidence
	cargo vet --locked

deny: ## Licence and advisory audit
	$(CARGO) deny --locked check

check: sync-check fmt-check lint guard-floats guard-deps guard-eval guard-rules test doc functional coverage vectors-check publish-check doc-coverage-check supply-chain-check vet deny ## Everything CI runs

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf .pytest_cache tests/.pytest_cache

matrix: ## Cross-architecture determinism matrix (needs podman)
	./tools/matrix.sh $(ARGS)
