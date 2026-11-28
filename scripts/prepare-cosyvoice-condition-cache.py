"""Insert after the existing stage observer without editing model or API."""
import ast
from pathlib import Path
import sys


def patch(source):
    if '_nai_condition_cache' in source:
        raise ValueError('ALREADY_PATCHED')
    tree = ast.parse(source)
    targets = [n for n in ast.walk(tree) if isinstance(n, ast.Assign)
               and any(isinstance(t, ast.Name) and t.id == '_nai_first_pcm' for t in n.targets)]
    if len(targets) != 1:
        raise ValueError('EXPECTED_ONE_OBSERVER')
    node = targets[0]
    indent = ' ' * node.col_offset
    lines = source.splitlines(keepends=True)
    lines[node.end_lineno:node.end_lineno] = [indent + line + '\n' for line in [
        'from nai_condition_cache import install as _nai_install_condition_cache',
        '_nai_condition_cache = _nai_install_condition_cache(cosyvoice)']]
    result = ''.join(lines)
    compile(result, 'cosyvoice-condition-cache-candidate', 'exec')
    return result


if __name__ == '__main__':
    Path(sys.argv[2]).write_text(patch(Path(sys.argv[1]).read_text(encoding='utf-8')), encoding='utf-8')
