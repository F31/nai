import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';import {
  DRIVER,
  ROOT_DIR,
  WORKSPACE_FILE,
  CI_WORKFLOW,
  ORPHAN_CAUSES,
  DISPOSITIONS,
  CONSUMER_KINDS,
  SCRIPT_DISPOSITIONS,
  packageDirOf,
  scriptErrors,
  driverSteps,
  driverExecutables,
  pnpmReferences,
  packageScripts,
  dispositionKey,
  stripSlashComments,
  stripYamlComments,
} from '../../scripts/contracts/npm-scripts.mjs';

/// 判据的输入**全部显式注入**：本模块自己不读磁盘（遍历工作区是 W75 的 F2 根因）。
///
/// 包目录与引用扫描面都由 **`.git/index`** 派生，不手写清单——手写的那一份会漂移，
/// 而「新增一个包却没人量它」正是本轮要挡的东西。
async function load() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));
  const tracked = [...index.paths];

  const trackedPackageJsons = tracked.filter(
    file => file === 'package.json' || /^[^/]+\/[^/]+\/package\.json$/.test(file),
  );
  const packageFiles = new Map();
  for (const file of trackedPackageJsons) {
    const dir = packageDirOf(file);
    packageFiles.set(dir, await readFile(file, 'utf8'));
  }

  // 引用扫描面：CI workflow + `scripts/**/*.mjs`，**排除判据模块目录**（那些是判据，不是消费者）。
  // `.md` **不在**这里——文档是给读者的指引，不是调用。
  const consumerPaths = [
    CI_WORKFLOW,
    ...tracked.filter(f => f.startsWith('scripts/') && f.endsWith('.mjs') && !f.startsWith('scripts/contracts/')),
  ];
  const consumerTexts = new Map();
  for (const file of consumerPaths) consumerTexts.set(file, await readFile(file, 'utf8'));

  // 证据落点：台账**自己声明**的那些文件 + 兜底集合。由台账派生 ⇒ 新增证据不用改这里。
  const evidencePaths = new Set([
    'docs/control.md',
    'docs/engine.md',
    'docs/engineering.md',
    'xtask/src/frontend.rs',
  ]);
  for (const entry of SCRIPT_DISPOSITIONS) {
    if (entry.docEvidence?.file) evidencePaths.add(entry.docEvidence.file);
    if (entry.equivalentTo?.file) evidencePaths.add(entry.equivalentTo.file);
  }
  const evidenceSources = new Map();
  for (const file of evidencePaths) {
    try {
      evidenceSources.set(file, await readFile(file, 'utf8'));
    } catch {
      // 缺文件不在这里假装通过：判据会以「cites X, which was not provided」具名。
    }
  }

  return {
    packageFiles,
    consumerTexts,
    driverText: await readFile(DRIVER, 'utf8'),
    evidenceSources,
    workspaceText: await readFile(WORKSPACE_FILE, 'utf8'),
    trackedPackageJsons,
  };
}

function withInputs(inputs, patch) {
  return { ...inputs, ...patch };
}

function dispositionsOf(...mutations) {
  const copy = structuredClone(SCRIPT_DISPOSITIONS);
  for (const mutate of mutations) mutate(copy);
  return copy;
}

function find(list, dir, script) {
  const entry = list.find(item => item.dir === dir && item.script === script);
  assert.ok(entry, `the fixture must contain ${dir} → "${script}"`);
  return entry;
}

/// 把某个包的 `package.json` 文本改一处（JSON 级替换，改不到就断言失败——注入必须是真注入）。
function patchManifest(text, from, to) {
  const next = text.replace(from, to);
  assert.notEqual(next, text, `the manifest injection must really change something: ${from}`);
  return next;
}

// ---------------------------------------------------------------------------
// 基线
// ---------------------------------------------------------------------------

test('the shipped baseline is clean, and every fact it needs was really provided', async () => {
  const inputs = await load();
  assert.deepEqual(scriptErrors(inputs), [], 'the baseline must carry no error');

  // 非空洞：注入进来的事实不许是空——否则每条判据都对空集恒真。
  assert.ok(inputs.packageFiles.size >= 3, 'every workspace manifest must be provided');
  assert.ok(inputs.trackedPackageJsons.length >= inputs.packageFiles.size, 'the tracked manifest set must cover them');
  assert.ok(SCRIPT_DISPOSITIONS.length > 0, 'the disposition table must not be empty');
  assert.ok(DISPOSITIONS.length > 0, 'the disposition closed set must not be empty');
  assert.ok(ORPHAN_CAUSES.length > 0, 'the cause closed set must not be empty');
  assert.ok(CONSUMER_KINDS.length > 0, 'the consumer closed set must not be empty');
  assert.ok(inputs.consumerTexts.size > 0, 'the reference scan surface must not be empty');

  const packages = packageScripts(inputs.packageFiles);
  let scripts = 0;
  for (const [, entry] of packages) scripts += Object.keys(entry.scripts).length;
  assert.equal(scripts, SCRIPT_DISPOSITIONS.length, 'every declared script must have exactly one disposition');

  // 每类处置都有实例（否则它那半套规则没有作用对象）。
  assert.ok(SCRIPT_DISPOSITIONS.some(e => e.disposition === 'invoked'), 'no invoked script');
  assert.ok(SCRIPT_DISPOSITIONS.some(e => e.disposition === 'orphan'), 'no orphan script');
  // 三格成因**都**被用到：闭集里有一格从没被用过时，它那一格形同不存在。
  for (const cause of ORPHAN_CAUSES) {
    assert.ok(
      SCRIPT_DISPOSITIONS.some(e => e.cause === cause),
      `the cause "${cause}" is declared but never exercised`,
    );
  }
});

/// 反例（W91：「每个限定 MUST 用反例验证它在拦什么」）：`frontend-gate.mjs` 只认
/// `node`／`tsc`／`vite`，而根包的 `test:control-smoke` 头 token 是 `cargo`。它**不该**红
/// ——因为它**不在驱动表里**，门控根本不经过它。拿驱动表那把尺子量它是用错了对象。
test('the driver executable limit applies to the driver table only', async () => {
  const inputs = await load();
  const packages = packageScripts(inputs.packageFiles);
  assert.equal(packages.get(ROOT_DIR).scripts['test:control-smoke'].startsWith('cargo '), true);
  assert.ok(!driverExecutables(inputs.driverText).includes('cargo'), 'cargo is not a driver executable');
  assert.deepEqual(scriptErrors(inputs), [], 'a non-driver script must not be measured by the driver limit');
});

test('workspace globs and the tracked manifests agree in both directions', async () => {
  const inputs = await load();
  const dirs = inputs.trackedPackageJsons.map(packageDirOf).filter(d => d !== ROOT_DIR);
  assert.ok(dirs.length > 0, 'the workspace must contain at least one package directory');
  assert.ok(dirs.includes('apps/desktop') && dirs.includes('packages/contracts-ts'));
});

// ---------------------------------------------------------------------------
// 闭世界
// ---------------------------------------------------------------------------

test('a script nobody dispositions is named', async () => {
  const inputs = await load();
  const packageFiles = new Map(inputs.packageFiles);
  packageFiles.set(
    ROOT_DIR,
    patchManifest(packageFiles.get(ROOT_DIR), '"typecheck":', '"typecheck:fast": "node scripts/nowhere.mjs",\n    "typecheck":'),
  );
  const errors = scriptErrors(withInputs(inputs, { packageFiles }));
  assert.ok(
    errors.some(error => error.includes('"typecheck:fast"') && error.includes('nobody dispositions')),
    `a new script with no disposition must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('a disposition whose script no longer exists is named', async () => {
  const inputs = await load();
  const packageFiles = new Map(inputs.packageFiles);
  packageFiles.set(
    ROOT_DIR,
    patchManifest(packageFiles.get(ROOT_DIR), ',\n    "test:engine-smoke": "cargo build -p nai-engine --locked --offline && node --experimental-strip-types scripts/engine-smoke.mjs"', ''),
  );
  const errors = scriptErrors(withInputs(inputs, { packageFiles }));
  assert.ok(
    errors.some(error => error.includes('"test:engine-smoke"') && error.includes('silently disappeared')),
    `a script deleted without touching the table must be named, saw ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// 消费者：注释不是引用；自引用不算消费者
// ---------------------------------------------------------------------------

test('an invocation that only survives in a comment is not a reference', async () => {
  const inputs = await load();
  const consumerTexts = new Map(inputs.consumerTexts);
  consumerTexts.set(
    CI_WORKFLOW,
    inputs.consumerTexts
      .get(CI_WORKFLOW)
      .replace('- run: pnpm spec:validate', '- run: echo "keeping the CI shape"  # pnpm spec:validate was here'),
  );
  const errors = scriptErrors(withInputs(inputs, { consumerTexts }));
  // 真引用被移进注释 ⇒ 剥注释后 marker 也不在了；`pnpm spec:validate` 于是不再有证据。
  assert.ok(
    errors.some(error => error.includes('spec:validate') && error.includes('comments are stripped')),
    `an invocation that only survives in a comment must not count, saw ${JSON.stringify(errors)}`,
  );
});

test('the comment strippers drop comments and keep strings', async () => {
  // 纯函数级：`"https://x"` 里的 `//` 不是注释（用行正则会吃掉字符串尾巴）。
  assert.ok(stripSlashComments('const a = "https://example.invalid/x"; // pnpm ghost').includes('https://example.invalid/x'));
  assert.ok(!stripSlashComments('const a = "https://example.invalid/x"; // pnpm ghost').includes('pnpm ghost'));
  assert.ok(!stripSlashComments('/* pnpm ghost */ const a = 1;').includes('pnpm ghost'));
  assert.ok(stripYamlComments('  - run: pnpm spec:validate  # pnpm ghost').includes('pnpm spec:validate'));
  assert.ok(!stripYamlComments('  - run: pnpm spec:validate  # pnpm ghost').includes('pnpm ghost'));
  // 引号里的 `#` 不是注释。
  assert.ok(stripYamlComments("  - name: '#1 thing'").includes('#1 thing'));
});

test('a self-reference cannot be its own consumer', async () => {
  const inputs = await load();
  const packageFiles = new Map(inputs.packageFiles);
  packageFiles.set(
    ROOT_DIR,
    patchManifest(packageFiles.get(ROOT_DIR), '"typecheck":', '"loop": "pnpm loop",\n    "typecheck":'),
  );
  const registered = dispositionsOf(list =>
    list.push({
      dir: ROOT_DIR,
      script: 'loop',
      disposition: 'invoked',
      consumer: { kind: 'pnpm', file: 'package.json', marker: 'pnpm loop' },
    }),
  );
  const errors = scriptErrors(withInputs(inputs, { packageFiles, registered }));
  assert.ok(
    errors.some(error => error.includes('"loop"') && error.includes('does not attribute')),
    `"loop": "pnpm loop" must not certify itself, saw ${JSON.stringify(errors)}`,
  );
});

test('a pnpm reference to a script that does not exist is named', async () => {
  const inputs = await load();
  const consumerTexts = new Map(inputs.consumerTexts);
  consumerTexts.set(
    CI_WORKFLOW,
    inputs.consumerTexts.get(CI_WORKFLOW).replace('run: pnpm spec:validate', 'run: pnpm spec:validate && pnpm ghost'),
  );
  const errors = scriptErrors(withInputs(inputs, { consumerTexts }));
  assert.ok(
    errors.some(error => error.includes('ghost') && error.includes('no script')),
    `a reference to a missing script must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('a filter that resolves to no package is named', async () => {
  const inputs = await load();
  const consumerTexts = new Map(inputs.consumerTexts);
  consumerTexts.set(
    CI_WORKFLOW,
    inputs.consumerTexts.get(CI_WORKFLOW).replace('run: pnpm spec:validate', 'run: pnpm --filter @nai/desktp typecheck'),
  );
  const errors = scriptErrors(withInputs(inputs, { consumerTexts }));
  assert.ok(
    errors.some(error => error.includes('@nai/desktp') && error.includes('no manifest declares')),
    `a filter that resolves to nothing must not read as "no consumer", saw ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// invoked 的两种形态
// ---------------------------------------------------------------------------

test('a driver claim that is not a real step is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, 'apps/desktop', 'test').consumer.step = { dir: 'apps/desktop', script: 'tests' };
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('"tests"') && error.includes('not a step')),
    `a step that is not in the driver has to be named, saw ${JSON.stringify(errors)}`,
  );
});

test('a consumer without a pair is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, 'apps/desktop', 'test').consumer = { kind: 'driver', step: 'apps/desktop:test' };
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('names no step') && error.includes('not a bare name')),
    `a bare step name must be refused, saw ${JSON.stringify(errors)}`,
  );
});

test('a pnpm consumer whose marker is missing is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, ROOT_DIR, 'spec:validate').consumer.marker = 'pnpm spec:validate --all';
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('spec:validate') && error.includes('comments are stripped')),
    `a citation that does not exist must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('a consumer kind outside the closed set is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, 'apps/desktop', 'test').consumer = { kind: 'magic' };
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('names none')),
    `an unknown consumer kind is a label, not a classification, saw ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// orphan 的成因与代价
// ---------------------------------------------------------------------------

test('a cause outside the closed set is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, ROOT_DIR, 'test:engine-smoke').cause = 'because';
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('"because"') && error.includes('not in the cause table')),
    `an unknown cause is a label, not a reason, saw ${JSON.stringify(errors)}`,
  );
});

test('a recursive delegation only covers the script of the same name', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    // 把 `typecheck` 的覆盖项改成 `build` 项：递归委托覆盖的是**同名**脚本。
    find(list, ROOT_DIR, 'typecheck').coveredBy = [
      { dir: 'packages/contracts-ts', script: 'build' },
      { dir: 'apps/desktop', script: 'build' },
    ];
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('only covers the script of the same name')),
    `a delegation to a recursive typecheck must not be certified by build steps, saw ${JSON.stringify(errors)}`,
  );
});

test('a covering step that is not in the driver is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, ROOT_DIR, 'typecheck').coveredBy = [
      { dir: 'packages/contracts-ts', script: 'typecheck' },
      { dir: 'apps/desktop', script: 'typecheck' },
      { dir: 'apps/ghost', script: 'typecheck' },
    ];
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('apps/ghost') && error.includes('not a step')),
    `coverage by a step that does not exist is not coverage, saw ${JSON.stringify(errors)}`,
  );
});

test('a script that is not a recursive delegation cannot claim driver coverage', async () => {
  const inputs = await load();
  const packageFiles = new Map(inputs.packageFiles);
  packageFiles.set(
    ROOT_DIR,
    patchManifest(packageFiles.get(ROOT_DIR), '"typecheck": "pnpm --recursive typecheck"', '"typecheck": "tsc --noEmit"'),
  );
  const errors = scriptErrors(withInputs(inputs, { packageFiles }));
  assert.ok(
    errors.some(error => error.includes('not a `pnpm --recursive <script>` delegation')),
    `a script doing something else is not covered by covering another, saw ${JSON.stringify(errors)}`,
  );
});

test('a manual-evidence file that is not a document is refused', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, ROOT_DIR, 'test:control-smoke').docEvidence = {
      file: 'scripts/control-smoke.mjs',
      marker: 'control-smoke',
    };
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('not a markdown file')),
    `the evidence for a manual script is a document, not a program, saw ${JSON.stringify(errors)}`,
  );
});

test('a manual script with no document blessing is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, ROOT_DIR, 'test:engine-smoke').docEvidence.marker = 'pnpm test:engine-smoke --watch';
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('nobody knows how to run')),
    `a manual script with no document is a script nobody knows how to run, saw ${JSON.stringify(errors)}`,
  );
});

test('an alias with no equivalent entry point is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    find(list, ROOT_DIR, 'check:frontend').equivalentTo.marker = 'pub const SCRIPT: &str = "scripts/ghost.mjs";';
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('the alias has no equivalent')),
    `an alias must name where the equivalent entry point is, saw ${JSON.stringify(errors)}`,
  );
});

test('an orphan with no cause at all is named', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => {
    delete find(list, 'apps/desktop', 'dev').cause;
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('"undefined"') && error.includes('not in the cause table')),
    `an orphan must carry a cause, saw ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// 驱动表接线：把门控的运行时拒绝静态化
// ---------------------------------------------------------------------------

test('a driver step naming a script the package does not declare is named', async () => {
  const inputs = await load();
  const driverText = inputs.driverText.replace("['apps/desktop', 'test'],", "['apps/desktop', 'tests'],");
  assert.notEqual(driverText, inputs.driverText, 'the driver injection must really change something');
  const errors = scriptErrors(withInputs(inputs, { driverText }));
  assert.ok(
    errors.some(error => error.includes('would fail at run time; it must fail here')),
    `a driver step with no script must be refused statically, saw ${JSON.stringify(errors)}`,
  );
});

test('a driver step whose program the gate cannot resolve is named', async () => {
  const inputs = await load();
  const packageFiles = new Map(inputs.packageFiles);
  packageFiles.set(
    'apps/desktop',
    patchManifest(packageFiles.get('apps/desktop'), '"typecheck": "tsc --noEmit"', '"typecheck": "eslint ."'),
  );
  const errors = scriptErrors(withInputs(inputs, { packageFiles }));
  assert.ok(
    errors.some(error => error.includes('eslint') && error.includes('not one of the executable names')),
    `the gate refuses an unknown program at run time; it must refuse it here, saw ${JSON.stringify(errors)}`,
  );
});

test('a driver step using shell syntax the gate never implements is named', async () => {
  const inputs = await load();
  const packageFiles = new Map(inputs.packageFiles);
  packageFiles.set(
    'apps/desktop',
    patchManifest(packageFiles.get('apps/desktop'), '"typecheck": "tsc --noEmit"', '"typecheck": "tsc --noEmit | tee log"'),
  );
  const errors = scriptErrors(withInputs(inputs, { packageFiles }));
  assert.ok(
    errors.some(error => error.includes('does not implement')),
    `the gate refuses unimplemented shell syntax at run time; it must refuse it here, saw ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// 包目录的闭世界
// ---------------------------------------------------------------------------

test('a package directory outside the workspace declaration is named', async () => {
  const inputs = await load();
  const trackedPackageJsons = [...inputs.trackedPackageJsons, 'native/toolbox/package.json'];
  const errors = scriptErrors(withInputs(inputs, { trackedPackageJsons }));
  assert.ok(
    errors.some(error => error.includes('native/toolbox') && error.includes('not matched by any')),
    `a package outside the workspace is not built by anyone, saw ${JSON.stringify(errors)}`,
  );
});

test('a workspace glob matching no package is named', async () => {
  const inputs = await load();
  const workspaceText = inputs.workspaceText.replace("- 'packages/*'", "- 'packages/*'\n  - 'examples/*'");
  assert.notEqual(workspaceText, inputs.workspaceText, 'the workspace injection must really change something');
  const errors = scriptErrors(withInputs(inputs, { workspaceText }));
  assert.ok(
    errors.some(error => error.includes('examples/*') && error.includes('matches no tracked package')),
    `an empty workspace entry makes its rule vacuous, saw ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// 缺输入与不可解析：具名失败，MUST NOT 静默降级
// ---------------------------------------------------------------------------

test('missing inputs are named rather than skipped', async () => {
  const inputs = await load();
  const cases = [
    [{ packageFiles: undefined }, 'package manifests were not provided'],
    [{ consumerTexts: undefined }, 'reference scan surface was not provided'],
    [{ driverText: '' }, 'driver source was not provided'],
    [{ evidenceSources: undefined }, 'evidence sources were not provided'],
    [{ workspaceText: '' }, WORKSPACE_FILE],
    [{ trackedPackageJsons: undefined }, 'tracked package manifests were not provided'],
  ];
  for (const [patch, needle] of cases) {
    const errors = scriptErrors(withInputs(inputs, patch));
    assert.equal(errors.length, 1, `${needle} must fail with one named error, saw ${JSON.stringify(errors)}`);
    assert.ok(errors[0].includes(needle), `expected ${needle}, saw ${errors[0]}`);
  }
});

test('an unparsable manifest fails by name instead of reading as "no scripts"', async () => {
  const inputs = await load();
  const packageFiles = new Map(inputs.packageFiles);
  packageFiles.set(ROOT_DIR, '{ this is not json');
  const errors = scriptErrors(withInputs(inputs, { packageFiles }));
  assert.ok(
    errors.some(error => error.includes('could not be parsed, so its scripts cannot be judged')),
    `an unparsable manifest must not be read as an empty script set, saw ${JSON.stringify(errors)}`,
  );
});

test('an unparsable driver fails by name instead of judging nothing', async () => {
  const inputs = await load();
  const errors = scriptErrors(withInputs(inputs, { driverText: 'const OTHER = [];' }));
  assert.ok(
    errors.some(error => error.includes('step table could not be parsed')),
    `an unparsable driver must not silently judge nothing, saw ${JSON.stringify(errors)}`,
  );
});

test('a cause table that no disposition exercises is named', async () => {
  const inputs = await load();
  // 把所有 orphan 都改签成 invoked（并让它们的消费者指向真东西）——成因闭集随后就没有作用对象。
  const registered = dispositionsOf(list => {
    for (const entry of list) {
      if (entry.disposition !== 'orphan') continue;
      delete entry.cause;
      delete entry.coveredBy;
      delete entry.equivalentTo;
      delete entry.docEvidence;
      entry.disposition = 'invoked';
      entry.consumer = { kind: 'driver', step: { dir: entry.dir, script: entry.script } };
    }
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('the cause rules are vacuous')),
    `an empty cause set is a disguised "no assertion", saw ${JSON.stringify(errors)}`,
  );
});

test('the disposition table rejects a duplicate pair', async () => {
  const inputs = await load();
  const registered = dispositionsOf(list => list.push({ ...find(list, 'apps/desktop', 'test') }));
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.ok(
    errors.some(error => error.includes('declared twice')),
    `a duplicate disposition must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('the disposition table keys pairs, not bare script names', async () => {
  // `typecheck` 在三个包里同名：键必须是配对，否则删掉一条会连带另外两条。
  const inputs = await load();
  const keys = SCRIPT_DISPOSITIONS.map(entry => dispositionKey(entry.dir, entry.script));
  assert.equal(new Set(keys).size, keys.length, 'the pair keys must be unique');
  const typechecks = SCRIPT_DISPOSITIONS.filter(entry => entry.script === 'typecheck');
  assert.equal(typechecks.length, 3, 'three packages declare `typecheck`; the table must key them apart');
  assert.equal(new Set(typechecks.map(entry => entry.dir)).size, 3);

  // 删掉其中一条 ⇒ 只点名**那一条**，另外两条不受影响。
  const registered = dispositionsOf(list => {
    const index = list.findIndex(entry => entry.dir === 'apps/desktop' && entry.script === 'typecheck');
    list.splice(index, 1);
  });
  const errors = scriptErrors(withInputs(inputs, { registered }));
  assert.equal(errors.length, 1, `only one pair must be named, saw ${JSON.stringify(errors)}`);
  assert.ok(errors[0].includes('apps/desktop') && errors[0].includes('"typecheck"'), errors[0]);
  assert.ok(!errors[0].includes('contracts-ts'), 'the sibling packages must not be implicated');
});

test('references are attributed to packages, and a recursive one targets every package', async () => {
  const inputs = await load();
  const packages = packageScripts(inputs.packageFiles);
  const scan = new Map(inputs.consumerTexts);
  for (const [dir, text] of inputs.packageFiles) {
    scan.set(dir === ROOT_DIR ? 'package.json' : `${dir}/package.json`, text);
  }
  const { refs } = pnpmReferences(scan, packages);
  const recursive = refs.filter(ref => ref.raw === 'pnpm --recursive typecheck');
  assert.equal(recursive.length, 1, 'the root delegation must be parsed');
  // 自引用被排除：根包自己**不**在目标里。
  assert.deepEqual(recursive[0].dirs.slice().sort(), ['apps/desktop', 'packages/contracts-ts']);
  assert.ok(!recursive[0].dirs.includes(ROOT_DIR), 'a recursive delegation must not consume its own package');
  // 驱动表解析是判定域：`covered-by-driver` 的证据必须真的在里面。
  const steps = driverSteps(inputs.driverText);
  assert.ok(steps.some(step => step.dir === 'apps/desktop' && step.script === 'build'));
});
