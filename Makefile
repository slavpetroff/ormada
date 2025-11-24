.PHONY: help fmt fix clippy check test clean all

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

clippy: ## Run clippy lints (enforces: no unwrap/expect/panic, must handle Results)
	@echo "🔍 Running clippy (strict mode: unwrap/expect/panic/unused results forbidden)..."
	@cargo clippy --all-targets --all-features

check: ## Run all checks (fmt, clippy, test)
	@echo "✅ Running all checks..."
	@cargo fmt --all -- --check
	@cargo clippy --all-targets --all-features
	@cargo test --all-targets

test: ## Run tests
	@echo "🧪 Running tests..."
	@cargo test --all-targets

clean: ## Clean build artifacts
	@echo "🧹 Cleaning..."
	@cargo clean

all: fix clippy test ## Format, fix, lint, and test
	@echo "✨ All done!"

# Convenience targets
f: fmt ## Alias for fmt
c: clippy ## Alias for clippy
t: test ## Alias for test
