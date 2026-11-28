"""Scoped diagnostics only. No global torch patch or third-party file edits."""
import ast
from contextlib import contextmanager
import inspect
import textwrap
import time
import types


def without_timing_sync(source):
    tree = ast.parse(textwrap.dedent(source))
    if len(tree.body) != 1 or not isinstance(tree.body[0], ast.FunctionDef) or tree.body[0].name != 'generate':
        raise ValueError('GENERATE_SOURCE')
    removed = {'sync': 0, 'print': 0}

    class Remove(ast.NodeTransformer):
        def visit_Expr(self, node):
            call = node.value
            if isinstance(call, ast.Call):
                if ast.unparse(call.func) == 'torch.cuda.synchronize':
                    if call.args or call.keywords:
                        raise ValueError('SYNC_ARGUMENTS')
                    removed['sync'] += 1
                    return ast.copy_location(ast.Pass(), node)
                if isinstance(call.func, ast.Name) and call.func.id == 'print':
                    if call.keywords or len(call.args) != 1 or not isinstance(call.args[0], ast.JoinedStr):
                        raise ValueError('PRINT_SIDE_EFFECTS')
                    if any(isinstance(n, (ast.Call, ast.NamedExpr)) for n in ast.walk(call.args[0])):
                        raise ValueError('PRINT_SIDE_EFFECTS')
                    removed['print'] += 1
                    return ast.copy_location(ast.Pass(), node)
            return self.generic_visit(node)

    tree = ast.fix_missing_locations(Remove().visit(tree))
    if not removed['sync']:
        raise ValueError('NO_TIMING_SYNC')
    return tree, removed


@contextmanager
def timing_sync_candidate(pipeline):
    original = pipeline.generate
    function = inspect.unwrap(original.__func__)
    if function.__closure__:
        raise ValueError('GENERATE_CLOSURE')
    tree, removed = without_timing_sync(inspect.getsource(function))
    namespace = {}
    exec(compile(tree, '<nai-generate-timing-diagnostic>', 'exec'), function.__globals__, namespace)
    pipeline.generate = types.MethodType(namespace['generate'], pipeline)
    try:
        yield removed
    finally:
        pipeline.generate = original


class Stages:
    def __init__(self, torch, pipeline):
        self.torch, self.pipeline = torch, pipeline
        self.originals, self.events = [], []

    def __enter__(self):
        targets = [('audio', self.pipeline, 'preprocess_audio'),
                   ('denoise', self.pipeline.model, 'forward'),
                   ('decode', self.pipeline.vae, 'decode'),
                   ('motion_encode', self.pipeline.vae, 'encode')]
        try:
            for name, owner, attribute in targets:
                original = getattr(owner, attribute)
                self.originals.append((owner, attribute, original))
                def timed(*args, _fn=original, _name=name, **kwargs):
                    if len(self.events) >= 256:
                        raise ValueError('PROFILE_LIMIT')
                    start = self.torch.cuda.Event(enable_timing=True)
                    end = self.torch.cuda.Event(enable_timing=True)
                    before = time.monotonic_ns()
                    start.record()
                    result = _fn(*args, **kwargs)
                    end.record()
                    self.events.append((_name, start, end, (time.monotonic_ns()-before)/1e6))
                    return result
                setattr(owner, attribute, timed)
            return self
        except BaseException:
            self.__exit__(None, None, None)
            raise

    def __exit__(self, *_):
        for owner, attribute, original in reversed(self.originals):
            setattr(owner, attribute, original)
        self.originals.clear()

    def report(self):
        result = {}
        for name, start, end, cpu in self.events:
            end.synchronize()
            stage = result.setdefault(name, {'calls': 0, 'cuda_stream_ms': 0., 'cpu_call_ms': 0.})
            stage['calls'] += 1
            stage['cuda_stream_ms'] += start.elapsed_time(end)
            stage['cpu_call_ms'] += cpu
        return result
