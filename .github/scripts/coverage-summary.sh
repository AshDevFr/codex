#!/usr/bin/env bash
# Render the coverage numbers from a CI run as Markdown, for the job summary and
# the pull-request comment.
#
# Usage: coverage-summary.sh <rust-summary.json> <web-coverage-summary.json>
#
# Either file may be missing: the report says which side did not produce one
# rather than failing, because a red report job would read as "coverage is
# broken" when the cause is a test job that failed for its own reasons.
#
# Inputs:
# - Rust: `cargo llvm-cov report --json --summary-only` (llvm-cov export format).
# - Web: Vitest's `json-summary` reporter (istanbul's coverage-summary.json).

set -euo pipefail

rust_json="${1:-}"
web_json="${2:-}"

pct() {
  # covered, total -> "12.3%", or "n/a" when there is nothing to cover.
  awk -v c="$1" -v t="$2" 'BEGIN { if (t == 0) print "n/a"; else printf "%.1f%%\n", 100 * c / t }'
}

echo "### Coverage"
echo

echo "#### Backend (Rust)"
echo
if [ -n "$rust_json" ] && [ -f "$rust_json" ]; then
  echo "| Crate | Lines | Functions | Regions |"
  echo "| --- | ---: | ---: | ---: |"

  # Group files by crate: crates/<name>/..., migration/..., else the root
  # `codex` binary crate. Paths are absolute on the runner, so match on the
  # segment rather than a prefix.
  jq -r '
    .data[0].files
    | map({
        crate: (
          if (.filename | test("/crates/[^/]+/")) then (.filename | capture("/crates/(?<c>[^/]+)/").c)
          elif (.filename | test("/migration/")) then "migration"
          else "codex"
          end
        ),
        s: .summary
      })
    | group_by(.crate)
    | map({
        crate: .[0].crate,
        lc: (map(.s.lines.covered) | add), lt: (map(.s.lines.count) | add),
        fc: (map(.s.functions.covered) | add), ft: (map(.s.functions.count) | add),
        rc: (map(.s.regions.covered) | add), rt: (map(.s.regions.count) | add)
      })
    | sort_by(.crate)[]
    | [.crate, .lc, .lt, .fc, .ft, .rc, .rt]
    | @tsv
  ' "$rust_json" | while IFS=$'\t' read -r crate lc lt fc ft rc rt; do
    echo "| \`$crate\` | $(pct "$lc" "$lt") | $(pct "$fc" "$ft") | $(pct "$rc" "$rt") |"
  done

  read -r lc lt fc ft rc rt < <(jq -r '
    .data[0].totals
    | [.lines.covered, .lines.count, .functions.covered, .functions.count,
       .regions.covered, .regions.count]
    | @tsv
  ' "$rust_json")
  echo "| **Total** | **$(pct "$lc" "$lt")** | **$(pct "$fc" "$ft")** | **$(pct "$rc" "$rt")** |"
else
  echo "_No report: the Rust coverage job did not produce one. Check its logs._"
fi
echo

echo "#### Frontend (web)"
echo
if [ -n "$web_json" ] && [ -f "$web_json" ]; then
  echo "| Lines | Statements | Functions | Branches |"
  echo "| ---: | ---: | ---: | ---: |"
  read -r lc lt sc st fc ft bc bt < <(jq -r '
    .total
    | [.lines.covered, .lines.total, .statements.covered, .statements.total,
       .functions.covered, .functions.total, .branches.covered, .branches.total]
    | @tsv
  ' "$web_json")
  echo "| $(pct "$lc" "$lt") | $(pct "$sc" "$st") | $(pct "$fc" "$ft") | $(pct "$bc" "$bt") |"
else
  echo "_No report: the frontend job did not produce one. Check its logs._"
fi
echo

echo "<sub>Line-level reports (lcov) are attached to the workflow run as the \`coverage-rust\` and \`coverage-web\` artifacts.</sub>"
