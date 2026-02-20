#!/usr/bin/env bash
set -euo pipefail

# extract-graph.sh — Build worktree merge graph from GitHub PRs + branches
# Deps: gh, jq, python3
# Output: viz/graph-data.json + viz/index.html (from template)

cd "$(dirname "$0")"

REPO="$(gh repo view --json nameWithOwner --jq '.nameWithOwner')"

echo "Fetching PRs for $REPO..."
PRS="$(gh pr list --state all --limit 200 \
  --json number,title,body,state,headRefName,baseRefName,additions,deletions,mergedAt,closedAt,url)"

echo "Fetching branches..."
BRANCHES="$(gh api "repos/$REPO/branches" --paginate --jq '.[].name')"

echo "Building graph..."
jq -n \
  --arg repo "$REPO" \
  --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson prs "$PRS" \
  --arg branches "$BRANCHES" \
'
def collect_branches:
  ($branches | split("\n") | map(select(length > 0 and (startswith("revert-") | not)))) as $br |
  ($prs | map(.headRefName, .baseRefName) | unique | map(select(startswith("revert-") | not))) as $pr_refs |
  ($br + $pr_refs) | unique | map(select(length > 0));

def parent_of:
  if . == "main" then null
  else (split(".") | if length <= 1 then ["main"] else .[:-1] end | join("."))
  end;

def label_of:
  if . == "main" then "main"
  else split(".") | last
  end;

collect_branches as $all_branches |

[
  $all_branches[] |
  . as $branch |
  {
    id: $branch,
    parent: ($branch | parent_of),
    depth: (if ($branch | startswith("revert-")) then 1 else ($branch | split(".") | length - 1) end),
    label: ($branch | label_of),
    prs: [
      $prs[] | select(.headRefName == $branch) |
      {
        number, title, body,
        state: (if .state == "MERGED" then "merged" elif .state == "CLOSED" then "closed" else "open" end),
        url, additions, deletions, mergedAt, closedAt
      }
    ],
    additions: ([$prs[] | select(.headRefName == $branch) | .additions] | add // 0),
    deletions: ([$prs[] | select(.headRefName == $branch) | .deletions] | add // 0)
  } |
  .status = (
    if .id == "main" then "root"
    elif (.prs | length) == 0 then "no-pr"
    elif (.prs | any(.state == "merged")) then "merged"
    elif (.prs | any(.state == "open")) then "open"
    else "closed"
    end
  )
] |

# Prune: keep only merged + root nodes (the "ensure parents" step below backfills any gaps)
map(select(.status == "merged" or .status == "root")) |

# Ensure parent nodes exist
(map(.id) | unique) as $existing |
(map(.parent) | unique | map(select(. != null and (. as $p | $existing | index($p) | not)))) as $missing |
. + [
  $missing[] |
  { id: ., parent: (. | parent_of), depth: (. | split(".") | length - 1),
    label: (. | label_of), prs: [], additions: 0, deletions: 0, status: "stub" }
] |

sort_by(.depth, .id) |

{
  meta: { repo: $repo, generated: $timestamp, node_count: length, pr_count: ($prs | length) },
  stats: {
    merged: [.[] | .prs[] | select(.state == "merged")] | length,
    closed: [.[] | .prs[] | select(.state == "closed")] | length,
    open:   [.[] | .prs[] | select(.state == "open")]   | length,
    total_additions: [.[] | .additions] | add,
    total_deletions: [.[] | .deletions] | add
  },
  nodes: .
}
' > graph-data.json

NODE_COUNT="$(jq '.meta.node_count' graph-data.json)"
PR_COUNT="$(jq '.meta.pr_count' graph-data.json)"
echo "Done. $NODE_COUNT nodes, $PR_COUNT PRs → graph-data.json"

# Assemble index.html from template by replacing %%GRAPH_DATA%% with the JSON
# Uses python3 for byte-exact replacement (jq -r mangles escape sequences in string values)
echo "Assembling index.html from template..."
python3 -c "
t = open('index.template.html', encoding='utf-8').read()
d = open('graph-data.json', encoding='utf-8').read()
open('index.html', 'w', encoding='utf-8').write(t.replace('%%GRAPH_DATA%%', d))
"
echo "Done. Open viz/index.html in a browser."
