_default:
    @just --list

binary_name := "env-verify"

format:
    @cargo fmt

lint: format
    @cargo clippy --all-targets --all-features -- -D warnings -D clippy::pedantic

build: lint
    @cargo build

build-arm: lint
    @cargo build --target=aarch64-unknown-linux-gnu

build-release: lint
    @cargo build

build-arm-release: lint
    @cargo build --target=aarch64-unknown-linux-gnu

test: lint
    @cargo test

generate-module-graph:
    @mkdir -p media
    @cargo modules dependencies --no-externs --no-fns --no-sysroot --no-traits --no-types --no-uses -p {{binary_name}} | dot -Tsvg > media/{{binary_name}}-modules.svg
    @cargo modules dependencies --no-sysroot --no-traits --no-types --no-uses -p {{binary_name}} | dot -Tsvg > media/{{binary_name}}-externs.svg
