"""Strict insertion at the unique existing AutoModel assignment; no other edits."""
import ast
from pathlib import Path
import sys


def patch(source):
    if '_nai_first_pcm' in source:raise ValueError('ALREADY_PATCHED')
    tree=ast.parse(source)
    targets=[n for n in ast.walk(tree) if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='cosyvoice' for t in n.targets)
        and isinstance(n.value,ast.Call) and isinstance(n.value.func,ast.Name) and n.value.func.id=='AutoModel']
    if len(targets)!=1:raise ValueError('EXPECTED_ONE_MODEL')
    n=targets[0];lines=source.splitlines(keepends=True);indent=' '*n.col_offset
    addition=[indent+line+'\n' for line in [
        'from nai_first_pcm import install as _nai_install',
        '_nai_first_pcm = _nai_install(cosyvoice, request_gate)',
        'load_wav = _nai_first_pcm.wrap_load(load_wav)']]
    lines[n.end_lineno:n.end_lineno]=addition
    result=''.join(lines);compile(result,'cosyvoice-first-pcm-candidate','exec')
    return result


if __name__=='__main__':
    Path(sys.argv[2]).write_text(patch(Path(sys.argv[1]).read_text(encoding='utf-8')),encoding='utf-8')
