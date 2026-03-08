#!/usr/bin/env python3
"""Build curated graph-data.json from GitHub PR data with synthetic phase groupings."""
import json
import re
import subprocess
import sys
from datetime import datetime

# === Fetch PR data ===
print("Fetching PRs...")
raw = subprocess.check_output([
    'gh', 'pr', 'list', '--state', 'all', '--limit', '200',
    '--json', 'number,title,body,state,headRefName,baseRefName,additions,deletions,mergedAt,closedAt,url'
], text=True)
all_prs = json.loads(raw)

# Only merged PRs with main.* branches (plus special cases)
skip_branches = {'main.ast-grep-research'}
prs = [p for p in all_prs if p['state'] == 'MERGED' and (p['headRefName'].startswith('main.') or p['headRefName'] in ('prelude-closure',)) and p['headRefName'] not in skip_branches]
prs.sort(key=lambda p: p['mergedAt'] or '')

print(f"  {len(prs)} merged PRs")

# === Reparenting: which flat branches belong under which synthetic parent ===
reparent = {
    # Phase 1: core-repr
    'main.types-and-datacon':    'main.core-repr',
    'main.pretty':               'main.core-repr',
    'main.serial':               'main.core-repr',
    'main.frame-and-utils':      'main.core-repr',
    'main.haskell-harness-impl': 'main.core-repr',
    'main.haskell-harness':      'main.core-repr',
    # Phase 2: core-eval
    'main.eval-strict-case':     'main.core-eval',
    'main.eval-thunks-joins':    'main.core-eval',
    # Phase 2: core-heap
    'main.heap-arena':           'main.core-heap',
    'main.gc-trace':             'main.core-heap',
    'main.gc-compact':           'main.core-heap',
    # Phase 2: core-bridge
    'main.bridge-scaffold':      'main.core-bridge',
    'main.bridge-derive':        'main.core-bridge',
    'main.haskell-macro':        'main.core-bridge',
    # Phase 2: core-testing
    'main.testing-generators':   'main.core-testing',
    'main.testing-oracle':       'main.core-testing',
    'main.testing-benchmarks':   'main.core-testing',
    # codegen-primops belongs under codegen
    'main.codegen-primops':      'main.codegen',
    # Extras: tide
    'main.tide-haskell':         'main.tide',
    'main.tide-parser':          'main.tide',
    # PRs that actually targeted main but should target their dot-parent
    'main.codegen.scaffold':     'main.codegen',
    'main.core-optimize.case-reduce': 'main.core-optimize',
    'main.core-optimize.beta-reduce': 'main.core-optimize',
    # Long tail: runtime
    'main.tidepool-runtime':     'main.runtime',
    'main.runtime-engine':       'main.runtime',
    'main.runtime-tests':        'main.runtime',
    'main.cache-tests':          'main.runtime',
    'main.eval-result':          'main.runtime',
    'prelude-closure':           'main.runtime',
    # Long tail: mcp
    'main.tidepool-mcp':         'main.mcp',
    'main.mcp-mature':           'main.mcp',
    'main.dogfood-handlers':     'main.mcp',
    'main.eval-resilience':      'main.mcp',
    'main.console-capture':      'main.mcp',
    # Long tail: polish
    'main.pre-publish':          'main.polish',
    'main.value-display':        'main.polish',
    'main.structured-errors':    'main.polish',
    'main.bridge-roundtrip':     'main.polish',
    'main.fix-gc-audit':         'main.polish',
    'main.pipeline-errors':      'main.polish',
    # --- Post-launch phases ---
    # Test coverage explosion (02-24)
    'main.test-heap-bridge':         'main.test-coverage',
    'main.test-host-fns':            'main.test-coverage',
    'main.test-render-bridge':       'main.test-coverage',
    'main.test-primops':             'main.test-coverage',
    'main.test-float-double':        'main.test-coverage',
    'main.test-cbor-serial':         'main.test-coverage',
    'main.test-case-expr':           'main.test-coverage',
    'main.test-bitwise-narrow':      'main.test-coverage',
    'main.fix-cache-test':           'main.test-coverage',
    'main.test-effect-cont':         'main.test-coverage',
    'main.test-int64-word64':        'main.test-coverage',
    'main.test-eval-edges':          'main.test-coverage',
    'main.test-pipeline-nursery':    'main.test-coverage',
    'main.test-bytearray-remaining': 'main.test-coverage',
    'main.test-error-sentinel':      'main.test-coverage',
    'main.test-comparison-complete': 'main.test-coverage',
    # Property-based testing wave (02-25)
    'main.proptest-host-fns':             'main.proptests',
    'main.proptest-optimizer-ext':        'main.proptests',
    'main.proptest-bridge-text':          'main.proptests',
    'main.proptest-cbor-roundtrip':       'main.proptests',
    'main.proptest-effect':               'main.proptests',
    'main.proptest-heap':                 'main.proptests',
    'main.proptest-jit-vs-eval':          'main.proptests',
    'main.consolidate-optimizer-proptests':'main.proptests',
    'main.bump-jit-proptests':            'main.proptests',
    'main.cbor-fuzz-proptests':           'main.proptests',
    'main.heap-gc-stress-proptests':      'main.proptests',
    'main.gc-pressure':                   'main.proptests',
    'main.effect-yield-resume':           'main.proptests',
    'main.partial-eval-proptest':         'main.proptests',
    'main.letrec-stress':                 'main.proptests',
    'main.generator-depth':              'main.proptests',
    # GC rewrite (02-25 – 03-08)
    'main.gc-raw-copy':           'main.gc-rewrite',
    'main.gc-scaffolding':        'main.gc-rewrite',
    'main.gc-wire':               'main.gc-rewrite',
    'main.gc-tests':              'main.gc-rewrite',
    'main.gc-forwarding-panic':   'main.gc-rewrite',
    'main.gc-heap-force-stale':   'main.gc-rewrite',
    # Bugfixes (02-21 – 03-04)
    'main.desugar-multi-return-primops': 'main.bugfix',
    'main.fix-text-split':               'main.bugfix',
    'main.fix-group-c-con-tag':          'main.bugfix',
    'main.fix-group-a-lit-zero':         'main.bugfix',
    'main.fix-group-b-heap-force':       'main.bugfix',
    'main.fix-tide-fs-handler':          'main.bugfix',
    'main.fix-q-silent-defaults':        'main.bugfix',
    # Features: aeson, ask, timeout, MCP improvements
    'main.aeson-prelude':        'main.features',
    'main.ask-effect':           'main.features',
    'main.eval-timeout':         'main.features',
    'main.mcp-string-schema':    'main.features',
    'main.improve-survey':       'main.features',
    'main.error-messages':       'main.features',
    'main.tco':                  'main.features',
    # Signal safety & crash resilience (03-04 – 03-06)
    'main.wu1-signal-protection-scope':  'main.signal-safety',
    'main.wu2-pthread-exit-fallback':    'main.signal-safety',
    'main.wu3-heap-bridge-validation':   'main.signal-safety',
    'main.wu4-crash-logging':            'main.signal-safety',
    'main.sigsegv-context':              'main.signal-safety',
    # Lazy thunks (03-05)
    'main.lazy-thunks-ws1-force':    'main.lazy-thunks',
    'main.lazy-thunks-ws2-codegen':  'main.lazy-thunks',
    'main.lazy-thunks-ws3-tests':    'main.lazy-thunks',
    # Hardening & cleanup (03-08)
    'main.optimizer-tree-utils':     'main.hardening',
    'main.emit-lam-thunk-dedup':     'main.hardening',
    'main.abort-to-poison':          'main.hardening',
    'main.mcp-dedup-misc':           'main.hardening',
    'main.primop-name-macro':        'main.hardening',
    'main.magic-constants':          'main.hardening',
    'main.emit-session-struct':      'main.hardening',
    'main.registry-raii-guard':      'main.hardening',
    'main.eval-limits':              'main.hardening',
    'main.primop-test-coverage':     'main.hardening',
    'main.mcp-cache-tests':          'main.hardening',
    'main.codegen-advanced-tests':   'main.hardening',
    'main.bridge-edge-tests':        'main.hardening',
    'main.prelude-text-tests':       'main.hardening',
    'main.heap-gc-tests':            'main.hardening',
    'main.sandbox-path-traversal':   'main.hardening',
    'main.translate-safety':         'main.hardening',
    'main.haskell-lib-fixes':        'main.hardening',
    'main.facade-runtime-fixes':     'main.hardening',
    'main.eval-safety':              'main.hardening',
    'main.host-fns-overflow':        'main.hardening',
    # Docs
    'main.update-claude-md':         'main.polish',
}

# === Build nodes from PRs ===
nodes = {}

def ensure_node(branch_id):
    if branch_id in nodes:
        return
    parts = branch_id.split('.')
    label = parts[-1] if len(parts) > 1 else branch_id
    parent = '.'.join(parts[:-1]) if len(parts) > 1 else None
    # Apply reparenting
    if branch_id in reparent:
        parent = reparent[branch_id]
    nodes[branch_id] = {
        'id': branch_id,
        'parent': parent,
        'depth': 0,  # computed later
        'label': label,
        'prs': [],
        'additions': 0,
        'deletions': 0,
        'status': 'merged' if branch_id != 'main' else 'root',
    }

# Root
ensure_node('main')

# Synthetic grouping nodes (these never had their own PRs)
synthetics = ['main.core-repr', 'main.core-eval', 'main.core-heap',
              'main.core-bridge', 'main.core-testing', 'main.tide',
              'main.runtime', 'main.mcp', 'main.polish',
              # Post-launch phases
              'main.test-coverage', 'main.proptests', 'main.gc-rewrite',
              'main.bugfix', 'main.features', 'main.signal-safety',
              'main.lazy-thunks', 'main.hardening']
for s in synthetics:
    ensure_node(s)

# Create nodes from PR head branches
for pr in prs:
    head = pr['headRefName']
    ensure_node(head)

    pr_data = {
        'number': pr['number'],
        'title': pr['title'],
        'body': pr['body'],
        'state': 'merged',
        'url': pr['url'],
        'additions': pr['additions'],
        'deletions': pr['deletions'],
        'mergedAt': pr['mergedAt'],
        'closedAt': pr['closedAt'],
    }
    nodes[head]['prs'].append(pr_data)
    nodes[head]['additions'] += pr['additions']
    nodes[head]['deletions'] += pr['deletions']

# === Give synthetic nodes a mergedAt from their latest child ===
for syn_id in synthetics:
    children = [n for n in nodes.values() if n['parent'] == syn_id]
    child_merges = []
    for c in children:
        for p in c['prs']:
            if p['mergedAt']:
                child_merges.append(p['mergedAt'])
    if child_merges:
        latest = max(child_merges)
        nodes[syn_id]['prs'] = [{
            'number': 0,
            'title': f'{nodes[syn_id]["label"]} (phase grouping)',
            'body': '',
            'state': 'merged',
            'url': '',
            'additions': 0,
            'deletions': 0,
            'mergedAt': latest,
            'closedAt': latest,
        }]

# === Fetch ALL first-parent commits on main and route to ideal parent ===
print("Fetching commits on main...")
git_log = subprocess.check_output(
    ['git', 'log', '--first-parent', '--format=%H %aI %s', 'main'],
    text=True, cwd='..'
)

# PR number → headRefName for routing merges to ideal parent
pr_branch = {}
for p in prs:
    pr_branch[p['number']] = p['headRefName']

known_pr_nums = set(pr_branch.keys())

# Find ideal parent for a branch (reparent map, then dot-parent)
def ideal_parent(branch):
    if branch in reparent:
        return reparent[branch]
    parts = branch.split('.')
    return '.'.join(parts[:-1]) if len(parts) > 1 else None

# Find the latest PR merge timestamp to exclude viz/post-project commits
latest_pr_merge = None
for p in prs:
    if p['mergedAt']:
        if latest_pr_merge is None or p['mergedAt'] > latest_pr_merge:
            latest_pr_merge = p['mergedAt']

# Route each commit to the right node's directCommits
parent_commits = {}  # node_id → [commit]
for line in git_log.strip().split('\n'):
    if not line.strip():
        continue
    parts = line.split(' ', 2)
    sha, timestamp, message = parts[0], parts[1], parts[2]

    # Skip commits after latest PR merge (viz commit etc.)
    if latest_pr_merge and timestamp > latest_pr_merge:
        continue

    # Extract PR number from squash merge "(#N)" or merge commit "Merge pull request #N"
    pr_num = None
    m = re.search(r'\(#(\d+)\)', message)
    if m and int(m.group(1)) in known_pr_nums:
        pr_num = int(m.group(1))
    if pr_num is None:
        m = re.search(r'Merge pull request #(\d+)', message)
        if m and int(m.group(1)) in known_pr_nums:
            pr_num = int(m.group(1))

    if pr_num is not None:
        # PR merge/squash → route to the ideal parent of the PR's branch
        branch = pr_branch[pr_num]
        target = ideal_parent(branch)
        kind = 'merge'
    else:
        # Direct TL work → stays on main
        target = 'main'
        kind = 'direct'

    # Only route to nodes that exist
    if target not in nodes:
        target = 'main'

    if target not in parent_commits:
        parent_commits[target] = []
    parent_commits[target].append({
        'sha': sha[:7],
        'message': message,
        'timestamp': timestamp,
        'kind': kind,
    })

# Attach sorted directCommits to each node
for node_id, commits in parent_commits.items():
    commits.sort(key=lambda c: c['timestamp'])
    nodes[node_id]['directCommits'] = commits

total_dc = sum(len(c) for c in parent_commits.values())
print(f"  {total_dc} commits routed to {len(parent_commits)} nodes:")
for nid in sorted(parent_commits.keys()):
    dc = parent_commits[nid]
    n_direct = sum(1 for c in dc if c['kind'] == 'direct')
    n_merge = sum(1 for c in dc if c['kind'] == 'merge')
    parts = []
    if n_direct: parts.append(f"{n_direct} direct")
    if n_merge: parts.append(f"{n_merge} merge")
    print(f"    {nid}: {', '.join(parts)}")

# === Compute depths from parent chain ===
def compute_depth(node_id):
    n = nodes[node_id]
    if n['parent'] is None:
        n['depth'] = 0
        return 0
    if n['parent'] not in nodes:
        # Create missing parent
        ensure_node(n['parent'])
    d = compute_depth(n['parent']) + 1
    n['depth'] = d
    return d

for nid in list(nodes.keys()):
    compute_depth(nid)

# === Assemble output ===
node_list = sorted(nodes.values(), key=lambda n: (n['depth'], n['id']))

total_add = sum(n['additions'] for n in node_list)
total_del = sum(n['deletions'] for n in node_list)
merged_count = sum(len(n['prs']) for n in node_list if n['prs'])

output = {
    'meta': {
        'repo': 'tidepool-heavy-industries/tidepool',
        'generated': datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ'),
        'node_count': len(node_list),
        'pr_count': len(prs),
    },
    'stats': {
        'merged': len(prs),
        'closed': sum(1 for p in all_prs if p['state'] == 'CLOSED'),
        'open': sum(1 for p in all_prs if p['state'] == 'OPEN'),
        'total_additions': total_add,
        'total_deletions': total_del,
    },
    'nodes': node_list,
}

with open('graph-data.json', 'w', encoding='utf-8') as f:
    json.dump(output, f, indent=2, ensure_ascii=False)

print(f"Done. {len(node_list)} nodes, {len(prs)} PRs → graph-data.json")
print()

# Show tree
def print_tree(nid, indent=0):
    n = nodes[nid]
    merge = ''
    if n['prs']:
        ts = [p['mergedAt'] for p in n['prs'] if p.get('mergedAt')]
        if ts:
            merge = f"  [{ts[-1][5:16]}]"
    kids = sorted([c['id'] for c in nodes.values() if c['parent'] == nid])
    kid_str = f"  ({len(kids)})" if kids else ""
    print(f"{'  ' * indent}{n['label']}{kid_str}{merge}")
    for kid in kids:
        print_tree(kid, indent + 1)

print("Tree:")
print_tree('main')
