"""Strictly add completion routes to an inspected service; candidates only."""
import argparse
import ast
from pathlib import Path


def prepare(source):
    tree = ast.parse(source)
    matches = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'pcm_response']
    if len(matches) != 1 or 'cosyvoice_completion_service' in source:
        raise ValueError('Unknown or already modified service')
    node = matches[0]
    if [a.arg for a in node.args.args] != ['model_output', 'stream']:
        raise ValueError('Unknown response signature')
    lines = source.splitlines(keepends=True)
    lines[node.lineno - 1:node.end_lineno] = [
        'from cosyvoice_completion_service import install as install_completion\n'
        'pcm_response = install_completion(app, request_gate, generate_data, lambda: cosyvoice.sample_rate)\n']
    output = ''.join(lines)
    compile(output, 'completion-service-candidate', 'exec')
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    with args.output.open('x', encoding='utf-8') as file:
        file.write(prepare(args.server.read_text(encoding='utf-8')))
