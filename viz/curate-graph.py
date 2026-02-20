#!/usr/bin/env python3
"""Build curated graph-data.json from GitHub PR data with synthetic phase groupings."""
import json
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

# Only merged PRs with main.* branches
prs = [p for p in all_prs if p['state'] == 'MERGED' and p['headRefName'].startswith('main.')]
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
    # Runtime
    'main.tidepool-runtime':     'main.tide',
    # PRs that actually targeted main but should target their dot-parent
    'main.codegen.scaffold':     'main.codegen',
    'main.core-optimize.case-reduce': 'main.core-optimize',
    'main.core-optimize.beta-reduce': 'main.core-optimize',
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
              'main.core-bridge', 'main.core-testing', 'main.tide']
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
