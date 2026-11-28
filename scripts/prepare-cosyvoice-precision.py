"""Prepare a precision CLI candidate from an inspected service; never edits source."""
import argparse
import ast
from pathlib import Path


def prepare(source):
    tree=ast.parse(source)
    main=[n for n in tree.body if isinstance(n,ast.If)
          and ast.unparse(n.test)=="__name__ == '__main__'"]
    models=[n for n in ast.walk(tree) if isinstance(n,ast.Call)
            and isinstance(n.func,ast.Name) and n.func.id=='AutoModel']
    if len(main)!=1 or len(models)!=1 or '--fp16' in source:
        raise ValueError('PRECISION_SOURCE_STRUCTURE')
    model=models[0]
    if (model not in list(ast.walk(main[0])) or model.args or len(model.keywords)!=1
            or model.keywords[0].arg!='model_dir'
            or ast.unparse(model.keywords[0].value)!='args.model_dir'):
        raise ValueError('PRECISION_MODEL_ARGUMENTS')
    parses=[n for n in main[0].body if isinstance(n,ast.Assign)
            and len(n.targets)==1 and ast.unparse(n.targets[0])=='args'
            and isinstance(n.value,ast.Call) and ast.unparse(n.value)=='parser.parse_args()']
    if len(parses)!=1 or parses[0].lineno>=model.lineno:
        raise ValueError('PRECISION_PARSE_POSITION')
    lines=source.splitlines(keepends=True)
    # Restrict replacement to the single verified call, not other strings/comments.
    if model.lineno!=model.end_lineno:raise ValueError('PRECISION_MODEL_LAYOUT')
    line=lines[model.lineno-1].encode('utf-8')
    lines[model.lineno-1]=(line[:model.col_offset]
        +b'AutoModel(model_dir=args.model_dir, fp16=args.fp16)'+line[model.end_col_offset:]).decode('utf-8')
    lines.insert(parses[0].lineno-1,"    parser.add_argument('--fp16', action='store_true', help='explicit half-precision candidate')\n")
    result=''.join(lines);compile(result,'precision-service-candidate','exec')
    return result


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    result=prepare(args.server.read_text(encoding='utf-8'))
    with args.output.open('x',encoding='utf-8') as out:out.write(result)
