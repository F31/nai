// 平台能力矩阵校验测试。
//
// 每个检查都配一个**非空洞守卫**：注入坏样本，断言该检查**确实抓到**。
// 否则「矩阵通过」与「检查恒真」无法区分。

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  MACOS_REQUIRED_RELEASE_LINES,
  SCHEMA_PATH,
  collectPlatformBranchFiles,
  deriveUnavailableReasons,
  readMatrix,
  validateMatrix,
} from '../../scripts/platform/matrix.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

const matrix = readMatrix(ROOT);

/** 深拷贝后改一处，用于注入。 */
const mutate = (edit) => {
  const copy = structuredClone(matrix);
  edit(copy);
  return copy;
};

const hasError = (errors, fragment) =>
  errors.some((error) => error.includes(fragment));

test('the committed matrix is internally consistent', () => {
  const errors = validateMatrix(ROOT, matrix);
  assert.deepEqual(errors, [], `matrix has errors:\n${errors.join('\n')}`);
});

test('reason codes are derived from the SDK enum, not copied here', () => {
  const { reasons, error } = deriveUnavailableReasons(ROOT);
  assert.equal(error, null, `derivation failed: ${error}`);
  assert.ok(reasons.length >= 5, `expected the full enum, derived only: ${reasons.join(', ')}`);
  for (const expected of ['no_official_interface', 'not_ported_to_target_os']) {
    assert.ok(reasons.includes(expected), `derived set is missing ${expected}`);
  }
  // 派生集合必须**恰好**是 Rust 侧 label() 的像，且不含 PascalCase 变体名。
  assert.ok(reasons.every((reason) => /^[a-z][a-z0-9_]*$/.test(reason)));
  assert.ok(!reasons.includes('NoOfficialInterface'));
});

test('the platform-branch scan is non-empty and finds the known branches', () => {
  // 若扫描恒为空，「源码分支 ⊆ 登记路径」这条核心门控会恒真——先钉住扫描本身有效。
  const branches = collectPlatformBranchFiles(ROOT);
  assert.ok(branches.size >= 10, `scan found only ${branches.size} files; the gate would be vacuous`);
  for (const known of [
    'crates/nai-security/src/lib.rs',
    'crates/nai-storage/src/paths.rs',
    'crates/nai-media-core/src/frame_slots/shared_ram.rs',
    'xtask/src/lib.rs',
  ]) {
    assert.ok(branches.has(known), `scan missed a known platform branch: ${known}`);
  }
  // 形态必须同时覆盖 windows / unix / target_os 三种写法。
  const texts = [...branches.keys()].map((file) => readFileSync(path.join(ROOT, file), 'utf8')).join('\n');
  assert.match(texts, /#\[cfg\(windows\)\]/);
  assert.match(texts, /#\[cfg\(unix\)\]/);
  assert.match(texts, /target_os = "linux"/);
});

test('every scanned platform branch is registered by a capability', () => {
  const errors = validateMatrix(ROOT, matrix);
  assert.equal(hasError(errors, 'is not registered by any capability'), false);
});

test('macOS is registered against Standard and Pro', () => {
  const macos = matrix.platforms.find((platform) => platform.id === 'macos');
  assert.ok(macos, 'macos platform is missing');
  for (const line of MACOS_REQUIRED_RELEASE_LINES) {
    assert.ok(macos.release_lines.includes(line), `macos must include ${line}`);
  }
  assert.deepEqual(MACOS_REQUIRED_RELEASE_LINES, ['r2', 'r3']);
  // 归属是登记，不是已支持。
  assert.notEqual(macos.status, 'supported');
});

test('every Windows-bound capability is unavailable on macOS', () => {
  const bound = matrix.capabilities.filter((capability) => capability.bound_to === 'windows');
  assert.ok(bound.length >= 2, 'expected at least the vault and the platform SDK to be Windows-bound');
  for (const capability of bound) {
    assert.notEqual(
      capability.states.macos.state,
      'supported',
      `${capability.id} must not be supported on macOS`,
    );
  }
  // 平台 SDK 是用户在 macOS 上要求如实不可得的对象。
  const sdk = matrix.capabilities.find((capability) => capability.id === 'platform-sdk');
  assert.equal(sdk.states.macos.state, 'explicit-unavailable');
  assert.equal(sdk.states.macos.reason, 'no_official_interface');
});

// ---------------------------------------------------------------------------
// 非空洞守卫
// ---------------------------------------------------------------------------

test('guard: an undeclared platform key is rejected', () => {
  const bad = mutate((copy) => {
    copy.capabilities[0].states.plan9 = { state: 'supported' };
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'states undeclared platform'));
});

test('guard: a missing platform is rejected', () => {
  const bad = mutate((copy) => {
    delete copy.capabilities[0].states.macos;
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'omits platform'));
});

test('guard: an unknown reason code is rejected', () => {
  const bad = mutate((copy) => {
    copy.capabilities[0].states.macos = { state: 'explicit-unavailable', reason: 'because_i_said_so' };
  });
  const errors = validateMatrix(ROOT, bad);
  assert.ok(hasError(errors, 'is not a UnavailableReason variant'));
});

test('guard: a PascalCase variant name is not accepted as a reason code', () => {
  const bad = mutate((copy) => {
    copy.capabilities[0].states.macos = {
      state: 'explicit-unavailable',
      reason: 'NotPortedToTargetOs',
    };
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'is not a UnavailableReason variant'));
});

test('guard: unavailable without a reason is rejected', () => {
  const bad = mutate((copy) => {
    copy.capabilities[0].states.macos = { state: 'explicit-unavailable' };
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'carries no reason'));
});

test('guard: supported carrying a reason is rejected', () => {
  const bad = mutate((copy) => {
    copy.capabilities[0].states.linux = { state: 'supported', reason: 'not_ported_to_target_os' };
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'must not carry a reason'));
});

test('guard: a dangling source file is rejected', () => {
  const bad = mutate((copy) => {
    copy.capabilities[0].source_files.push('crates/nai-media-core/src/frame_slots/ghost.rs');
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'does not exist'));
});

test('guard: an unregistered platform branch is rejected', () => {
  const injected = new Map([['crates/nai-kernel/src/new_platform_path.rs', 3]]);
  const errors = validateMatrix(ROOT, matrix, injected);
  assert.ok(hasError(errors, 'is not registered by any capability'));
  assert.ok(hasError(errors, 'new_platform_path.rs'));
});

test('guard: macOS losing Pro is rejected', () => {
  const bad = mutate((copy) => {
    const macos = copy.platforms.find((platform) => platform.id === 'macos');
    macos.release_lines = ['r2'];
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'must include "r3"'));
});

test('guard: a Windows-bound capability claiming macOS support is rejected', () => {
  const bad = mutate((copy) => {
    const sdk = copy.capabilities.find((capability) => capability.id === 'platform-sdk');
    sdk.states.macos = { state: 'supported' };
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'claims supported on macos'));
});

test('guard: a source-based capability with no evidence is rejected', () => {
  const bad = mutate((copy) => {
    copy.capabilities[0].source_files = [];
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'requires at least one source file'));
});

test('guard: a baseline capability registering files is rejected', () => {
  const bad = mutate((copy) => {
    const sdk = copy.capabilities.find((capability) => capability.id === 'platform-sdk');
    sdk.source_files = ['crates/nai-plugin-sdk/src/connector.rs'];
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'must not register source files'));
});

test('guard: a duplicate platform id is rejected', () => {
  const bad = mutate((copy) => {
    copy.platforms.push({ ...copy.platforms[0] });
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'is declared more than once'));
});

test('guard: an invented release line is rejected', () => {
  const bad = mutate((copy) => {
    copy.platforms.find((platform) => platform.id === 'macos').release_lines.push('r4');
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'is not one of'));
});

test('guard: a malformed target triple is rejected', () => {
  const bad = mutate((copy) => {
    copy.platforms.find((platform) => platform.id === 'macos').target_triples = ['macos'];
  });
  assert.ok(hasError(validateMatrix(ROOT, bad), 'is not <arch>-<vendor>-<os>'));
});

test('the matrix data file and its schema both exist', () => {
  const schema = JSON.parse(readFileSync(path.join(ROOT, SCHEMA_PATH), 'utf8'));
  assert.equal(schema.type, 'object');
  assert.ok(schema.required.includes('platforms') && schema.required.includes('capabilities'));
  // reason 刻意不写 enum —— 合法值由源码派生，避免第二处定义。
  const serialized = JSON.stringify(schema);
  assert.equal(serialized.includes('no_official_interface'), false);
});
