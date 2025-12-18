.PHONY: help fmt fix clippy check test test-verbose bench doc clean all release ci

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-15s %s\n", $$1, $$2}'

fmt: ## Format code with rustfmt
	@echo "📝 Formatting code..."
	@cargo fmt --all

fix: ## Auto-fix warnings (including removing unused imports)
	@echo "🔧 Auto-fixing warnings and removing unused imports..."
	@cargo fix --allow-dirty --allow-staged
	@cargo fmt --all

clippy: ## Run clippy lints (strict mode)
	@echo "🔍 Running clippy..."
	@cargo clippy --all-targets --all-features -- -D warnings

check: ## Run all checks (fmt, clippy, test)
	@echo "✅ Running all checks..."
	@cargo fmt --all -- --check
	@cargo clippy --all-targets --all-features -- -D warnings
	@cargo test --all-targets

test: ## Run tests
	@echo "🧪 Running tests..."
	@cargo test --all-targets

test-verbose: ## Run tests with output
	@echo "🧪 Running tests (verbose)..."
	@cargo test --all-targets -- --nocapture

bench: ## Run benchmarks
	@echo "📊 Running benchmarks..."
	@cargo bench

doc: ## Build documentation
	@echo "📚 Building documentation..."
	@cargo doc --no-deps --all-features

clean: ## Clean build artifacts
	@echo "🧹 Cleaning..."
	@cargo clean

all: fix clippy test ## Format, fix, lint, and test
	@echo "✨ All done!"

release: ## Build release version
	@echo "📦 Building release..."
	@cargo build --release

ci: ## Run CI checks (used in GitHub Actions)
	@echo "🔄 Running CI checks..."
	@cargo fmt --all -- --check
	@cargo clippy --all-targets --all-features -- -D warnings
	@cargo test --all-targets
	@cargo doc --no-deps --all-features

# Convenience aliases
f: fmt
c: clippy
t: test
b: bench
d: doc
