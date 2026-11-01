# Start here: plain-language usage and examples.
default: help

# Explain setup, running, building, and checks.
help:
    @cat scripts/dev/help.txt

# Install dependencies and preserve existing .env files.
install target="all":
    @bash scripts/dev/task.sh install {{quote(target)}}

# Build a component or everything (SDKs first).
build target="all":
    @bash scripts/dev/task.sh build {{quote(target)}}

# Run backend/dashboard separately, or together in mprocs.
run target="all":
    @bash scripts/dev/task.sh run {{quote(target)}}

# Validate code, or use `check tools [component]` for setup diagnostics.
check target="all" component="all":
    @bash scripts/dev/task.sh check {{quote(target)}} {{quote(component)}}

# Generate a database migration; review the generated SQL before running it.
migrate name:
    just --justfile crates/condite-core/justfile migrate {{quote(name)}}
