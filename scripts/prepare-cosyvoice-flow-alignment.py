"""Prepare an opt-in Flow reference policy after the existing cache."""
import argparse
import ast
from pathlib import Path


def prepare(source):
    if '_nai_flow_alignment' in source:raise ValueError('ALIGNMENT_ALREADY_PREPARED')
    tree=ast.parse(source)
    main=[n for n in tree.body if isinstance(n,ast.If) and ast.unparse(n.test)=="__name__ == '__main__'"]
    if len(main)!=1:raise ValueError('ALIGNMENT_MAIN')
    parses=[n for n in main[0].body if isinstance(n,ast.Assign) and ast.unparse(n)=='args = parser.parse_args()']
    caches=[n for n in main[0].body if isinstance(n,ast.Assign)
            and ast.unparse(n)=='_nai_condition_cache = _nai_install_condition_cache(cosyvoice)']
    if len(parses)!=1 or len(caches)!=1 or parses[0].lineno>=caches[0].lineno:
        raise ValueError('ALIGNMENT_CACHE_BOUNDARY')
    lines=source.splitlines(keepends=True)
    addition="""    if args.flow_reference_min_seconds is not None:
        from nai_flow_alignment import install as _nai_install_flow_alignment
        from atexit import register as _nai_register_flow_cleanup
        _nai_flow_alignment_close = _nai_install_flow_alignment(cosyvoice,
            minimum_seconds=args.flow_reference_min_seconds, max_bytes=args.flow_reference_max_bytes)
        _nai_register_flow_cleanup(_nai_flow_alignment_close)
"""
    lines[caches[0].end_lineno:caches[0].end_lineno]=[addition]
    lines.insert(parses[0].lineno-1,
        "    parser.add_argument('--flow-reference-min-seconds', type=float, default=None)\n"
        "    parser.add_argument('--flow-reference-max-bytes', type=int, default=16777216)\n")
    output=''.join(lines);compile(output,'flow-alignment-candidate','exec')
    return output


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    result=prepare(args.server.read_text(encoding='utf-8'))
    with args.output.open('x',encoding='utf-8') as out:out.write(result)
