from pathlib import Path
import sys
from types import SimpleNamespace as NS
from unittest.mock import Mock, patch
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[2]/'scripts'))
from avatar_compile_maintenance import Maintenance
from avatar_compile import install, require_evidence

class MaintenanceTests(unittest.TestCase):
    def test_lazy_wrapper_is_not_compiled_execution_evidence(self):
        for counters, count in [({}, 0), ({'unique_graphs': 1}, 0), ({}, 5)]:
            with self.assertRaisesRegex(RuntimeError, 'NO_COMPILED_EXECUTION'):
                require_evidence(counters, count)
        require_evidence({'unique_graphs': 1}, 2)

    def test_ready_only_after_body_and_timer_join(self):
        path = Mock()
        with patch('avatar_compile_maintenance.threading.Timer') as timer:
            with Maintenance(path):
                self.assertEqual(path.write_text.call_count, 1)
            timer.return_value.cancel.assert_called_once()
            timer.return_value.join.assert_called_once()
        self.assertIn('maintenance_ready_not_production', path.write_text.call_args.args[0])

    def test_failure_and_timeout_never_ready(self):
        path = Mock()
        with patch('avatar_compile_maintenance.threading.Timer'):
            with self.assertRaises(ValueError):
                with Maintenance(path):
                    raise ValueError()
        self.assertIn('maintenance_failed', path.write_text.call_args.args[0])
        with patch('avatar_compile_maintenance.os._exit') as exit_process:
            Maintenance(path).expired()
            exit_process.assert_called_once_with(124)
        self.assertIn('maintenance_timeout', path.write_text.call_args.args[0])
        for n in (0, 241):
            with self.assertRaises(ValueError): Maintenance(path, n)

    def test_compile_fixed_shapes_without_suppressed_errors(self):
        torch = NS(_dynamo=NS(config=NS(suppress_errors=True)), compile=Mock(side_effect=lambda fn, **_: fn))
        pipeline = NS(model=Mock(), vae=NS(model=NS(encode=Mock(), decode=Mock())))
        install(torch, pipeline)
        self.assertFalse(torch._dynamo.config.suppress_errors)
        self.assertEqual(torch.compile.call_count, 3)
        for call in torch.compile.call_args_list:
            self.assertEqual(call.kwargs, {'dynamic': False})

    def test_explicit_sdpa_boundary_preserves_operator(self):
        operator = Mock()
        torch = NS(_dynamo=NS(config=NS(suppress_errors=True), disallow_in_graph=Mock()),
                   nn=NS(functional=NS(scaled_dot_product_attention=operator)), compile=Mock(side_effect=lambda fn, **_: fn))
        pipeline = NS(model=Mock(), vae=NS(model=NS(encode=Mock(), decode=Mock())))
        install(torch, pipeline, True)
        torch._dynamo.disallow_in_graph.assert_called_once_with(operator)

    def test_block_scope_does_not_compile_whole_model(self):
        torch = NS(_dynamo=NS(config=NS(suppress_errors=True)), compile=Mock(side_effect=lambda fn, **_: fn))
        blocks = [Mock(), Mock()]
        pipeline = NS(model=NS(blocks=list(blocks)), vae=NS(model=NS(encode=Mock(), decode=Mock())))
        install(torch, pipeline, blocks=True)
        self.assertEqual(torch.compile.call_count, 4)
        self.assertEqual([c.args[0] for c in torch.compile.call_args_list[:2]], blocks)

if __name__ == '__main__': unittest.main()
