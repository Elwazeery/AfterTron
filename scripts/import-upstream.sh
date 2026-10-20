#!/usr/bin/env bash
set -euo pipefail
: "${UPSTREAM_URL:?Pin approved upstream URL}"
: "${UPSTREAM_SHA:?Pin exact approved upstream SHA}"
: "${PRODUCT_NAME:?Set intended new product}"
if [[ "$(git branch --show-current)" != "main" ]]; then echo 'Expected main checkout' >&2; exit 2; fi
if [[ ! "$UPSTREAM_SHA" =~ ^[0-9a-f]{40}$ ]]; then echo 'Upstream SHA must be full 40 hex chars' >&2; exit 2; fi
if git remote get-url tron-upstream >/dev/null 2>&1; then
  [[ "$(git remote get-url tron-upstream)" == "$UPSTREAM_URL" ]] || { echo 'Wrong upstream remote' >&2; exit 2; }
else
  git remote add tron-upstream "$UPSTREAM_URL"
fi
git fetch --no-tags tron-upstream main
actual="$(git rev-parse FETCH_HEAD)"
if [[ "$actual" != "$UPSTREAM_SHA" ]]; then
  echo "Upstream moved: expected $UPSTREAM_SHA, actual $actual. Refusing unreviewed import." >&2
  exit 2
fi
if git merge-base --is-ancestor "$UPSTREAM_SHA" HEAD; then
  echo 'Upstream commit already merged; no action.'
  exit 0
fi
# Preserve full source Git ancestry and avoid replacing existing Tron docs.
git merge -s ours --allow-unrelated-histories --no-commit "$UPSTREAM_SHA"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
# Restore all source files except conflicting Tron entrypoints and unreviewed CI.
git ls-tree -r -z --name-only "$UPSTREAM_SHA" | python3 -c '
import sys
for raw in sys.stdin.buffer.read().split(b"\0"):
    if not raw: continue
    if raw in (b"README.md", b"TODO.md", b"AGENTS.md"): continue
    if raw.startswith(b".github/workflows/"): continue
    sys.stdout.buffer.write(raw + b"\0")
' > "$tmp"
git restore --source="$UPSTREAM_SHA" --staged --worktree --pathspec-from-file="$tmp" --pathspec-file-nul
mkdir -p docs/upstream
for name in README.md TODO.md AGENTS.md; do
  if git cat-file -e "$UPSTREAM_SHA:$name" 2>/dev/null; then
    git show "$UPSTREAM_SHA:$name" > "docs/upstream/$name"
  fi
done
cat > docs/upstream/IMPORT-RECEIPT.md <<EOF
# ${PRODUCT_NAME} upstream source import

- Upstream Git repository: ${UPSTREAM_URL}
- Imported upstream commit: \`${UPSTREAM_SHA}\`
- Method: two-parent Git merge preserving original upstream ancestry
- Target: existing main; Tron README/TODO/AGENTS retained
- Original docs: mirrored under docs/upstream; source LICENSE files retained
- Upstream GitHub Actions workflows: deliberately excluded until reviewed
- Native build/rebrand/MCP completeness: NOT VERIFIED BY SOURCE IMPORT
- Provenance check: \`git merge-base --is-ancestor ${UPSTREAM_SHA} HEAD\`
EOF
git add -A
git commit -m "chore(${PRODUCT_NAME}): import pinned upstream with complete Git ancestry"
git push origin HEAD:main
