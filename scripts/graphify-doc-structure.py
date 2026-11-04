"""Bounded, local Graphify Markdown structure. No semantic/model backend.

This developer tool combines the current code graph with explicit documentation
structure using Graphify's own extractor, resolver, builder and exporter.
"""
import argparse
import hashlib
import json
from pathlib import Path

from graphify.build import build
from graphify.export import to_json
from graphify.extract import extract
from graphify.paths import write_json_atomic


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root, output = args.root.resolve(), args.output.resolve()
    if not output.is_relative_to(root / 'target-windows' / 'tool-cache'):
        raise ValueError('Graph output must remain in the project tool-cache')
    sources = [
        'docs/design.md',
        'docs/digital-host-agent-design-V4.8.md',
        'docs/v48-development-plan.md',
        'docs/development-plan.md',
        'docs/architecture/v48-boundaries-and-command-inventory.md',
        'docs/adr/0151-v48-adoption-plan.md',
        'docs/adr/0185-manual-review-next-draft.md',
        'docs/reports/2026-10-27-v48-08-manual-review-draft.md',
    ]
    paths = [root / relative for relative in sources]
    for path in paths:
        if not path.is_file() or path.stat().st_size > 2 * 1024 * 1024:
            raise ValueError('Documentation source missing or exceeds 2 MiB: ' + str(path))
    code = json.loads((output / 'graph.json').read_text(encoding='utf-8'))
    edges = code.get('links', code.get('edges', []))
    docs = extract(paths, cache_root=output / 'document-structure-cache', root=root,
                   parallel=False, max_workers=1,
                   resolution_context_nodes=code['nodes'],
                   resolution_context_edges=edges)
    if docs.get('input_tokens', 0) or docs.get('output_tokens', 0):
        raise ValueError('Structural extraction must not consume model tokens')
    graph = build([{'nodes': code['nodes'], 'edges': edges}, docs],
                  root=root, directed=True, dedup=False)
    graph.graph['nai_document_analysis'] = 'headings, explicit links and symbol mentions only; no semantic extraction'
    target = output / 'architecture-docs.json'
    to_json(graph, {}, str(target), force=True)
    summary = {
        'graphify_version': '0.9.96', 'semantic_analysis': False,
        'code_graph_sha256': hashlib.sha256((output / 'graph.json').read_bytes()).hexdigest(),
        'document_count': len(paths), 'document_nodes': len(docs['nodes']),
        'document_edges': len(docs['edges']),
        'combined_nodes': graph.number_of_nodes(), 'combined_edges': graph.number_of_edges(),
        'sources': [{'path': p.relative_to(root).as_posix(),
                     'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in paths],
    }
    write_json_atomic(output / 'document-structure-summary.json', summary, indent=2, ensure_ascii=False)
    print(json.dumps({k: v for k, v in summary.items() if k != 'sources'}, ensure_ascii=False))


if __name__ == '__main__':
    main()
