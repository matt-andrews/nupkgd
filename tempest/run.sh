#!/usr/bin/env bash
# Builds nupkgd, serves the fixtures on port 8080 and runs the Tempest integration suite in Docker.
#
# Usage: tempest/run.sh [tempest test args...]      e.g. tempest/run.sh --debug --run search.spec.yml
# Environment:
#   TEMPEST_IMAGE     Docker image to run (default: mattisthegreatest/tempest:1)
#   NUPKGD_NO_BUILD   set to 1 to skip `cargo build`
# Exits with Tempest's exit code (0 pass, 1 failures, 2 flaky with --strict).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
image="${TEMPEST_IMAGE:-mattisthegreatest/tempest:1}"
port=5555 # fixed: tempest/.env points Tempest at host.docker.internal:8080

if [[ "${NUPKGD_NO_BUILD:-0}" != "1" ]]; then
  (cd "$repo" && cargo build)
fi

bin="$repo/target/debug/nupkgd"
[[ -x "$bin" ]] || bin="$bin.exe"
[[ -x "$bin" ]] || { echo "nupkgd binary not found under $repo/target/debug" >&2; exit 1; }

log="$(mktemp)"
"$bin" start --dir "$here/fixtures" --bind "$port" >"$log" 2>&1 &
server=$!
cleanup() {
  kill "$server" 2>/dev/null || true
  wait "$server" 2>/dev/null || true
  rm -f "$log"
}
trap cleanup EXIT

ready=0
for _ in $(seq 1 120); do
  if ! kill -0 "$server" 2>/dev/null; then
    echo "nupkgd exited early:" >&2
    cat "$log" >&2
    exit 1
  fi
  # 127.0.0.1 on purpose: nupkgd binds IPv4 only.
  if curl -fsS "http://127.0.0.1:$port/healthz" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 0.25
done
if [[ "$ready" != 1 ]]; then
  echo "nupkgd did not answer /healthz on port $port within 30 seconds:" >&2
  cat "$log" >&2
  exit 1
fi
echo "nupkgd is serving $here/fixtures on port $port"

# Git Bash on Windows rewrites POSIX paths in arguments; hand Docker a native path instead.
mount_src="$here"
if command -v cygpath >/dev/null 2>&1; then
  mount_src="$(cygpath -w "$here")"
fi

set +e
# --add-host makes host.docker.internal resolve on Linux engines too; Docker Desktop
# already provides it, where the flag is harmless.
MSYS_NO_PATHCONV=1 docker run --rm \
  --add-host host.docker.internal:host-gateway \
  -v "${mount_src}:/etc/tests" \
  "$image" test "$@"
code=$?
set -e

exit "$code"
