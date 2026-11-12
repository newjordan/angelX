"""CPU-only recovery contracts. No model weights, endpoints, or Modal imports."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


def load(name):
    path = Path(__file__).with_name(name + ".py")
    if not path.exists():
        return None
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


regime = load("regime")
batching = load("regime_batch")


def reply(point):
    return '<tool_call>' + json.dumps({"arguments": {"coordinate": point}}) + '</tool_call>'


class GroundEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(hasattr(regime, "ground_record"), "replayable grounding evidence not implemented")
        self.row = {"family": "shopping", "screen": "a", "group": "g", "target": "Cart",
                    "box": [0, 0, 99, 79]}

    def test_bare_coordinates_and_truncated_json_fail_closed(self):
        for text in ('"coordinate": [100, 200]', '<tool_call>{"arguments":{"coordinate":[100,200]'):
            with self.subTest(text=text):
                with self.assertRaises(ValueError):
                    regime.parse_ground_reply(text)
                row = regime.ground_record(self.row, "hands-v1", "test", (100, 80), [text])
                self.assertIsNone(row["first_point"])
                self.assertIsNone(row["point"])
                self.assertFalse(row["cascade"])
                regime.verify_ground_record(row)

    def test_raw_first_and_failed_zoom_are_replayable(self):
        texts = [reply([500, 500]), 'no usable point']
        row = regime.ground_record(self.row, "hands-v1", "test", (100, 80), texts)
        self.assertEqual(row["first_point"], row["point"])
        self.assertEqual([x["raw_reply"] for x in row["looks"]], texts)
        self.assertTrue(row["first"])
        self.assertTrue(row["cascade"])
        self.assertEqual(row["successful_looks"], 1)
        regime.verify_ground_record(row)

    def test_full_success_requires_all_three_looks(self):
        with self.assertRaisesRegex(ValueError, "incomplete"):
            regime.ground_record(self.row, "candidate", "test", (100, 80), [reply([500, 500])])
        with self.assertRaisesRegex(ValueError, "after.*failure"):
            regime.ground_record(self.row, "candidate", "test", (100, 80), ['bad', reply([500, 500])])

    def test_flags_coordinates_raw_replies_and_crops_are_checked(self):
        row = regime.ground_record(self.row, "candidate", "test", (100, 80), [reply([500, 500])] * 3)
        mutations = [lambda r: r.update(first=False), lambda r: r.update(cascade=False),
                     lambda r: r.update(first_point=[0, 0]), lambda r: r.update(point=[0, 0]),
                     lambda r: r["looks"][1].update(crop=[0, 0, 1, 1]),
                     lambda r: r["looks"][0].update(raw_reply='"coordinate":[500,500]')]
        for mutate in mutations:
            bad = copy.deepcopy(row)
            mutate(bad)
            with self.subTest(mutate=mutate), self.assertRaisesRegex(ValueError, "evidence"):
                regime.verify_ground_record(bad)

    def test_production_rounding_is_away_from_zero_and_tiny_crops_stay_nonempty(self):
        self.assertEqual(regime.zoom_window([7.5, 7.5], (20, 20), 2), [3, 3, 10, 10])
        self.assertEqual(regime.zoom_window([0, 0], (1, 1), 3), [0, 0, 1, 1])

    def test_prompt_normalizes_click_prefix_and_trailing_periods(self):
        self.assertEqual(regime.prompt("system", "  cLiCk   Cart...  "), regime.prompt("system", "Cart"))


class BatchTests(unittest.TestCase):
    def setUp(self):
        self.assertIsNotNone(batching, "true minibatch helpers not implemented")
        import torch
        self.torch = torch
        torch.set_num_threads(1)

    def sample(self, ids, n_ans, patches=4, marker=1):
        t = self.torch
        return {"input_ids": t.tensor([ids]), "n_ans": n_ans,
                "pixel_values": t.full((patches, 2), float(marker)),
                "image_grid_thw": t.tensor([[1, 2, patches // 2]]),
                "patch_positions": t.arange(patches * 3).reshape(patches, 3)}

    def test_real_batch_pads_text_masks_answers_and_concatenates_images_in_order(self):
        b = batching.collate_samples([self.sample([1, 2, 3, 4], 2),
                                     self.sample([5, 6, 7], 1, 8, 2)], pad_token_id=0)
        self.assertEqual(b["input_ids"].tolist(), [[1, 2, 3, 4], [5, 6, 7, 0]])
        self.assertEqual(b["attention_mask"].tolist(), [[1, 1, 1, 1], [1, 1, 1, 0]])
        self.assertEqual(b["labels"].tolist(), [[-100, -100, 3, 4], [-100, -100, 7, -100]])
        self.assertEqual(b["image_grid_thw"].tolist(), [[1, 2, 2], [1, 2, 4]])
        self.assertEqual(b["pixel_values"].shape, (12, 2))
        self.assertEqual(b["pixel_values"][:, 0].tolist(), [1.] * 4 + [2.] * 8)
        self.assertEqual(b["patch_positions"].shape, (12, 3))

    def test_invalid_answer_masks_and_patch_counts_are_rejected(self):
        for n in (0, 4, 5):
            with self.subTest(n=n), self.assertRaisesRegex(ValueError, "answer"):
                batching.collate_samples([self.sample([1, 2, 3, 4], n)], pad_token_id=0)
        bad = self.sample([1, 2, 3, 4], 2)
        bad["pixel_values"] = bad["pixel_values"][:-1]
        with self.assertRaisesRegex(ValueError, "patch"):
            batching.collate_samples([bad], pad_token_id=0)

    def test_batched_answer_loss_and_gradients_equal_token_weighted_singles(self):
        t = self.torch
        t.manual_seed(7)
        samples = [self.sample([1, 2, 3, 4], 2), self.sample([5, 6, 7], 1)]
        b = batching.collate_samples(samples, pad_token_id=0)
        hidden = t.randn(2, 4, 3, requires_grad=True)
        head = t.nn.Linear(3, 9, bias=False)
        loss, tokens = batching.answer_loss_sum(hidden, b["labels"], head)
        expected = t.nn.functional.cross_entropy(head(t.stack([hidden[0, 1], hidden[0, 2], hidden[1, 1]])).float(),
                                                 t.tensor([3, 4, 7]), reduction="sum")
        self.assertEqual(tokens, 3)
        t.testing.assert_close(loss, expected)
        (loss / tokens).backward()
        grad = head.weight.grad.clone()
        self.assertEqual(hidden.grad[0, 3].abs().sum().item(), 0)
        self.assertEqual(hidden.grad[1, 2:].abs().sum().item(), 0)
        head.zero_grad()
        total = 0
        for i, sample in enumerate(samples):
            one = batching.collate_samples([sample], pad_token_id=0)
            value, n = batching.answer_loss_sum(hidden.detach()[i:i+1, :one["labels"].shape[1]], one["labels"], head)
            value.backward()
            total += n
        t.testing.assert_close(head.weight.grad / total, grad)

    def test_partial_accumulation_is_normalized_and_applied_not_discarded(self):
        t = self.torch
        param = t.nn.Parameter(t.tensor([1.]))
        opt = t.optim.SGD([param], lr=0.1)
        (param * 6).sum().backward()
        norm = batching.finish_accumulation(opt, [param], answer_tokens=3, max_grad_norm=100)
        self.assertAlmostEqual(norm, 2.)
        self.assertAlmostEqual(param.item(), 0.8, places=6)
        self.assertIsNone(param.grad)
        with self.assertRaises(ValueError):
            batching.finish_accumulation(opt, [param], answer_tokens=0)

    def test_nonfinite_gradients_never_step(self):
        t = self.torch
        param = t.nn.Parameter(t.tensor([1.]))
        opt = t.optim.SGD([param], lr=0.1)
        param.grad = t.tensor([float('nan')])
        with self.assertRaisesRegex(ValueError, "finite"):
            batching.finish_accumulation(opt, [param], answer_tokens=1)
        self.assertEqual(param.item(), 1.)

    def test_training_loop_updates_all_examples_including_short_final_window(self):
        self.assertTrue(hasattr(batching, 'train_epoch'), 'tested minibatch training loop not implemented')
        t = self.torch
        param = t.nn.Parameter(t.tensor([1.]))
        opt = t.optim.SGD([param], lr=.1)
        batches = [{'input_ids': t.zeros((n, 2)), 'labels': t.zeros((n, 2))} for n in (2, 2, 1)]
        def loss_on(batch):
            n = batch['input_ids'].shape[0]
            return param.sum() * n, n
        records = []
        result = batching.train_epoch(batches, loss_on, opt, [param], accumulation_steps=2,
                                      deadline=float('inf'), log=records.append, max_grad_norm=100)
        self.assertEqual((result['steps'], result['samples'], result['answer_tokens']), (2, 5, 5))
        self.assertEqual([r['window_samples'] for r in records], [4, 1])
        self.assertAlmostEqual(param.item(), .8, places=6)

    def test_training_loop_flushes_pending_window_at_time_limit(self):
        self.assertTrue(hasattr(batching, 'train_epoch'), 'tested minibatch training loop not implemented')
        t = self.torch
        param = t.nn.Parameter(t.tensor([1.]))
        opt = t.optim.SGD([param], lr=.1)
        clock_value = [0.]
        batches = [{'input_ids': t.zeros((2, 2))}] * 3
        def loss_on(batch):
            clock_value[0] += 2
            return param.sum() * 2, 2
        result = batching.train_epoch(batches, loss_on, opt, [param], accumulation_steps=8,
                                      deadline=1., log=lambda r: None, max_grad_norm=100,
                                      clock=lambda: clock_value[0])
        self.assertEqual((result['samples'], result['steps']), (2, 1))
        self.assertAlmostEqual(param.item(), .9, places=6)


class TrainerWiringTests(unittest.TestCase):
    """Import the real launcher with a recording-only Modal stand-in; never connects."""
    def test_launcher_rejects_missing_budget_and_scope_before_remote_call(self):
        import types
        from unittest.mock import patch
        calls = []
        class Chain:
            def __getattr__(self, name):
                return lambda *a, **k: self
        class App:
            def __init__(self, name):
                pass
            def local_entrypoint(self):
                return lambda f: f
            def function(self, **kwargs):
                def deco(f):
                    f.remote = lambda **kw: calls.append(kw)
                    return f
                return deco
        fake = types.SimpleNamespace(App=App, Image=types.SimpleNamespace(debian_slim=lambda **k: Chain()),
                                     Volume=types.SimpleNamespace(from_name=lambda n: Chain()))
        with patch.dict('sys.modules', {'modal': fake}):
            module = load('modal_regime')
        self.assertTrue(hasattr(module, 'run_approved'), 'local pre-allocation approval gate not implemented')
        common = dict(run='new-run', budget_usd=0., steam_scope='', approved=False,
                      compute_rate_usd_per_hour=10., dataset_sha256='a' * 64)
        for change in ({}, {'approved': True}, {'approved': True, 'budget_usd': 20},
                       {'approved': True, 'budget_usd': 20, 'steam_scope': 'native-client'},
                       {'approved': True, 'budget_usd': .01, 'steam_scope': 'public-store'},
                       {'approved': True, 'budget_usd': 20, 'steam_scope': 'public-store', 'run': '../old'}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                module.run_approved(**{**common, **change})
        self.assertEqual(calls, [])
        valid = {**common, 'approved': True, 'budget_usd': 20., 'steam_scope': 'public-store'}
        for change in ({'compute_rate_usd_per_hour': 0}, {'budget_usd': float('nan')},
                       {'budget_usd': float('inf')}, {'batch_size': 0},
                       {'dataset_sha256': ''}, {'max_runtime_minutes': 86}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                module.run_approved(**{**valid, **change})
        self.assertEqual(module.run_approved(**valid)['dry_run'], True)
        self.assertEqual(calls, [])
        module.run_approved(**valid, dry_run=False)
        self.assertEqual(len(calls), 1)  # recording stub, never the real Modal SDK
        self.assertEqual(calls[0]['dataset_sha256'], 'a' * 64)

    def test_trainer_uses_full_replies_real_batches_and_reloads_before_candidate_eval(self):
        import ast
        path = Path(__file__).with_name('modal_regime.py')
        tree = ast.parse(path.read_text())
        campaign = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'campaign')
        self.assertFalse(any(isinstance(n, ast.Name) and n.id == 'StopAtPoint' for n in ast.walk(campaign)),
                         'regex early stopping still used')
        source = ast.unparse(campaign)
        self.assertIn('collate_samples', source)
        self.assertIn('train_epoch', source)
        self.assertIn('ground_record', source)
        self.assertNotIn('batch_size=None', source)
        self.assertNotIn('subset(test', source)
        self.assertIn('max_new_tokens=1024', source)
        reload_pos = source.index('model = load_checkpoint(final_path')
        self.assertGreater(source.index("candidate_test = evaluate("), reload_pos)


class ProductionParityTests(unittest.TestCase):
    """Compile the actual four production functions, not a second handwritten oracle."""
    @classmethod
    def setUpClass(cls):
        cls.root = Path(__file__).resolve().parents[4]
        cls.source = cls.root / 'cockpit/src/agent/tools/hands.rs'
        source = cls.source.read_text()
        names = ('parse_ground_reply', 'frame_to_px', 'zoom_window', 'ground_user_text')
        pieces = []
        for name in names:
            start = source.index('pub(crate) fn ' + name + '(')
            end = source.index('\n}', start) + 2
            pieces.append(source[start:end])
        deps = cls.root / 'cockpit/target/debug/deps'
        libs = sorted(deps.glob('libserde_json-*.rlib'))
        if not libs:
            raise RuntimeError('production parity blocked: cached serde_json rlib not available')
        cls.tmp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.tmp.cleanup)
        path = Path(cls.tmp.name)
        main = r'''
fn main() {
    use std::io::Read;
    let mut input = String::new(); std::io::stdin().read_to_string(&mut input).unwrap();
    let cases: Value = serde_json::from_str(&input).unwrap();
    let mut results = Vec::new();
    for c in cases.as_array().unwrap() {
        let r = match c["kind"].as_str().unwrap() {
            "reply" => match parse_ground_reply(c["text"].as_str().unwrap()) {
                Ok(p) => serde_json::json!({"point": [p.0,p.1]}), Err(_) => serde_json::json!({"point": null}) },
            "prompt" => serde_json::json!({"text": ground_user_text(c["text"].as_str().unwrap())}),
            "geometry" => {
                let p = (c["point"][0].as_f64().unwrap(),c["point"][1].as_f64().unwrap());
                let size = (c["size"][0].as_u64().unwrap() as u32,c["size"][1].as_u64().unwrap() as u32);
                let px = frame_to_px(p,size);
                let win = zoom_window(px,size,c["zoom"].as_u64().unwrap() as u32);
                serde_json::json!({"point": [px.0,px.1],"crop": [win.0,win.1,win.2,win.3]}) },
            _ => unreachable!()
        };
        results.push(r);
    }
    println!("{}",serde_json::to_string(&results).unwrap());
}
'''
        (path / 'oracle.rs').write_text('use serde_json::Value;\n' + '\n'.join(pieces) + main)
        cls.binary = path / 'oracle'
        subprocess.run(['rustc', '--edition=2021', str(path / 'oracle.rs'), '--extern', f'serde_json={libs[0]}',
                        '-L', f'dependency={deps}', '-o', str(cls.binary)], check=True, capture_output=True, text=True)

    def oracle(self, cases):
        run = subprocess.run([str(self.binary)], input=json.dumps(cases), check=True, capture_output=True, text=True)
        return json.loads(run.stdout)

    def test_reply_corpus_matches_actual_rust_parser(self):
        self.assertTrue(hasattr(regime, 'parse_ground_reply'), 'production parser port not implemented')
        valid = reply([123, 456])
        texts = ['', '"coordinate":[1,2]', valid, valid.removesuffix('</tool_call>'),
                 '<tool_call>bad</tool_call>' + valid, reply([True, 2]) + valid,
                 reply([1]) + valid, reply([1, 2, 3]) + valid, reply([-1, 2]) + valid,
                 '<tool_call>{"arguments":{"coordinate":[1e2,2.5]}}</tool_call>',
                 '<tool_call>{"arguments":{"coordinate":[null,0]}}</tool_call>' + valid,
                 '<tool_call>{"arguments":{"coordinate":[NaN,2]}}</tool_call>' + valid,
                 '<tool_call>{"bad":1e999,"arguments":{"coordinate":[1,2]}}</tool_call>' + valid,
                 '<tool_call>[1,2]</tool_call>' + valid, '<tool_call>{"coordinate":[1,2]}</tool_call>' + valid]
        texts += [valid[:i] for i in range(len(valid))]
        for x in (0, 999, 999.001, -0.001, 7.5, 1e20):
            for y in (0, 999, -1):
                texts += [reply([x, y]), reply([x, y]) + valid]
        wanted = self.oracle([{'kind': 'reply', 'text': text} for text in texts])
        for text, expected in zip(texts, wanted):
            with self.subTest(text=text):
                try:
                    point = regime.parse_ground_reply(text)
                except ValueError:
                    point = None
                self.assertEqual(point, expected['point'])

    def test_prompt_and_crop_geometry_match_actual_rust(self):
        self.assertTrue(hasattr(regime, 'frame_to_px'), 'production geometry port not implemented')
        cases = [{'kind': 'prompt', 'text': x} for x in (' Cart. ', 'cLiCk  Cart...', 'Click ', 'double click', 'CliCK Cart', '')]
        cases += [{'kind': 'geometry', 'point': p, 'size': s, 'zoom': z}
                  for p in ([0, 0], [999, 999], [499.5, 499.5], [7.5, 3.5])
                  for s in ((1, 1), (20, 20), (1919, 1079), (1920, 1080)) for z in (0, 2, 3)]
        for case, expected in zip(cases, self.oracle(cases)):
            with self.subTest(case=case):
                if case['kind'] == 'prompt':
                    self.assertEqual(regime.ground_user_text(case['text']), expected['text'])
                else:
                    px = regime.frame_to_px(case['point'], case['size'])
                    self.assertEqual(regime.zoom_window(px, case['size'], case['zoom']), expected['crop'])
                    for got, want in zip(px, expected['point']):
                        self.assertAlmostEqual(got, want, places=10)


if __name__ == '__main__':
    unittest.main()
