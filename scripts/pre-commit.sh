#!/bin/bash
# Pre-commit checks to verify the code against the CI pipeline requirements.
# Runs build, formatting, clippy, and tests.

set -e

# Default to debug mode for faster local iteration, but support --release to match CI exactly.
PROFILE=""
TEST_PROFILE=""
BUILD_ARGS=""

while [[ "$#" -gt 0 ]]; do
  case $1 in
    --release)
      PROFILE="--release"
      TEST_PROFILE="--profile release"
      BUILD_ARGS="--release"
      shift
      ;;
    *)
      echo "Unknown parameter: $1"
      echo "Usage: $0 [--release]"
      exit 1
      ;;
  esac
done

echo "========================================="
echo "       Running Pre-Commit Checks         "
echo "========================================="

echo "--> Running cargo fmt --check"
cargo fmt --check

echo "--> Running cargo build ${BUILD_ARGS} --all-features"
cargo build ${BUILD_ARGS} --all-features

echo "--> Running cargo clippy"
cargo clippy --all-targets --all-features -- -D warnings

echo "--> Running cargo test ${TEST_PROFILE}"
if [ -n "$TEST_PROFILE" ]; then
  cargo test ${TEST_PROFILE}
else
  cargo test
fi

echo "========================================="
echo "   All checks passed successfully!       "
echo "========================================="
