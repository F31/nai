/// Node 测试文件的**可达性闭世界**：仓库里每个测试文件 MUST 被某个**登记站点**覆盖。
///
/// 这个模块存在的理由：门控是靠**逐条列举**（`engineering_checks` 里手写 `node --test <path>`）
/// 加**两个 glob**（`tests/ui/*.test.mjs`、`apps/desktop/tests/*.test.mjs`）把测试接上电的。
/// 「列举」的盲区恰好是**它的补集**：往仓库里放一个 `tests/kernel/foo.test.mjs`，**没有任何地方
/// 会注意到它**——`cargo xtask check --profile full` 照样 `EXIT=0`、目标数不变、用例数不变。
///
/// 实测（W84 探针）：一个**已在 `.git/index` 里**、含 `assert.fail(...)` 的测试文件放进
/// `tests/kernel/`，**完整门控仍然全绿**，且该文件的用例名在门控日志里**零出现**。
/// ⇒ 与「未登记 = 不跑」不同，这里是**静默的不跑**：没有任何信号，连「忽略了一个文件」都没有。
///
/// 于是补法与 W81／W82 同形：给「哪些测试文件在跑」补一条**闭世界**——
/// **在册（已跟踪）的测试文件 == 覆盖集 ∪ 未覆盖台账**，且两者**不相交**。
///
/// 三条纪律照抄自前轮：
/// 1. **覆盖集由 marker 原文解析得出，MUST NOT 另行声明**（W80：手写的那一份会漂移）。
///    `TEST_REGISTRATION_SITES` 只登记「站点在哪里、写了什么」，覆盖集是**算出来的**。
/// 2. **「已声明」≠「已执行」**（W75）：站点声明的 marker MUST 在其 host 里**逐字存在**，
///    否则它是一条**假契约**——名字在表里，线上什么都没有。
/// 3. **闭世界 MUST 在每个层级上成立**（W82）：文件之上还有**扩展名**这一层。
///    只认 `.test.mjs` 而不管「有没有别种测试扩展名」，等于把盲区搬到更少人看的一层。

/// **被识别的测试文件扩展名**。逐字写死，MUST NOT 由「仓库里现有文件」派生——
/// 派生出的「现有扩展名 == 清单」是恒真式。
///
/// 实测全仓只有 `.test.mjs` 一种（`.test.js`／`.cjs`／`.ts`／`.mts` 一个都没有）。
/// 只列 `.test.mjs` 是本轮**有意**的选择：新增一种扩展名会被 `extensionErrors` 具名点名，
/// 而不是静静地不被任何站点扫到。
export const TEST_EXTENSIONS = ['.test.mjs'];

/// 站点所属的**门控层级**。闭集，理由与 `DISPOSITIONS` 一致：让「这个站点在哪儿跑」可判定。
/// - `portable`：`engineering_checks`（`portable` 与 `full` 都从这里开始）；
/// - `full`：仅 `full` 的**前端门控**（`frontend::check`）；
/// - `ci`：不在 `check` profile 里，只由 CI 工作流步骤跑。
export const TEST_TIERS = ['portable', 'full', 'ci'];

/// **登记站点**：一个「在某处把某组测试接上电」的**具体落点**。
///
/// 每条 `{id, host, marker, base, tier}`：
/// - `host`：写有该引用的**文件**（相对仓库根）；
/// - `marker`：MUST 在 `host` 里**逐字出现**的字符串（这是「已声明」的凭据）；
/// - `base`：`marker` 里的相对路径所**相对**的目录（`package.json` 的 glob 是相对该包目录的）；
/// - `tier`：∈ `TEST_TIERS`。
///
/// **逐字写死**：这份表是「站点在哪里」的**独立声明**。覆盖集由它**解析**得出，因此
/// 「表被删空 ⇒ 覆盖集为空 ⇒ 闭世界全红」是**想要的**方向（而不是恒真）。
export const TEST_REGISTRATION_SITES = [
  {
    id: 'xtask-registry',
    host: 'xtask/src/registry.rs',
    marker: 'tests/registry.test.mjs',
    base: '.',
    tier: 'portable',
  },
  {
    id: 'xtask-engineering-schema',
    host: 'xtask/src/lib.rs',
    marker: 'tests/contracts/schema.test.mjs',
    base: '.',
    tier: 'portable',
  },
  {
    id: 'xtask-engineering-platform',
    host: 'xtask/src/lib.rs',
    marker: 'tests/platform/platform-matrix.test.mjs',
    base: '.',
    tier: 'portable',
  },
  {
    id: 'xtask-contracts-suite',
    host: 'xtask/src/schema.rs',
    marker: 'tests/contracts/descriptor.test.mjs',
    base: '.',
    tier: 'ci',
  },
  {
    id: 'ci-contract-suite',
    host: '.github/workflows/foundation.yml',
    marker: 'tests/contracts/descriptor.test.mjs',
    base: '.',
    tier: 'ci',
  },
  {
    id: 'root-test-ui',
    host: 'package.json',
    marker: 'tests/ui/*.test.mjs',
    base: '.',
    tier: 'full',
  },
  {
    id: 'desktop-test',
    host: 'apps/desktop/package.json',
    marker: 'tests/*.test.mjs',
    base: 'apps/desktop',
    tier: 'full',
  },
];

/// **未覆盖台账**：**在册**（已跟踪）却**不被任何站点覆盖**的测试文件，每条必须带**理由**。
///
/// 为什么是台账而不是「豁免名单」：台账**不是豁免**（要对得上、要相称、要自清理）。
/// `reachabilityErrors` 会验证每条**确实**未被覆盖——一条已经可达的条目会**立刻红**
/// （否则台账会变成「加进去就再也不用管」的垃圾桶）。
///
/// 当前**为空**：实测 9 个测试文件全部落在某个站点的覆盖集里。
/// 空台账是**合法**的，但它使「两集不相交」退化成恒真——因此本条**必须**与
/// 「覆盖集非空」这条断言**成对**存在（否则「全不覆盖」也能过）。
export const LOCALLY_UNREACHABLE = [];

/// 「看起来像测试」的判定：路径里含 `.test.` 或 `.spec.` 段。
///
/// 它比 `isTestFile` **宽**——宽出来的那一块正是**扩展名层的补集**：
/// `foo.test.ts` 会被 `looksLikeTest` 抓到、却不被 `isTestFile` 认，于是被点名。
function looksLikeTest(filePath) {
  return /(?:^|\/)[^/]*[.-](?:test|spec)\.[^/]+$/.test(filePath);
}

/// `filePath` 是否为一个**被识别的**测试文件（扩展名 ∈ `TEST_EXTENSIONS`）。
export function isTestFile(filePath) {
  return TEST_EXTENSIONS.some(extension => String(filePath).endsWith(extension));
}

/// 把 `marker` 展开成「匹配哪些**已跟踪**路径」的判定。纯函数：只看 marker 与 base。
/// - 无通配 ⇒ `kind: 'file'`，只匹配**恰好** `base/marker`；
/// - 含 `*`／`?` ⇒ `kind: 'glob'`，`*` 不跨 `/`（与 shell 的 `*` 语义一致）。
export function markerMatcher(site) {
  const base = site.base === '.' || site.base === '' ? '' : `${site.base}/`;
  const target = `${base}${site.marker}`;
  if (!/[*?]/.test(site.marker)) {
    return { kind: 'file', target, matches: filePath => filePath === target };
  }
  const pattern = target
    .replace(/[.+^${}()|[\]\\]/g, '\\$&')
    .replace(/\*/g, '[^/]*')
    .replace(/\?/g, '[^/]');
  const regexp = new RegExp(`^${pattern}$`);
  return { kind: 'glob', target, matches: filePath => regexp.test(filePath) };
}

/// 一个站点在**已跟踪路径集**上覆盖到的全部文件。
///
/// 刻意**不**用 `readdir`：覆盖集必须按**仓库成员**（`.git/index`）计算，与工作区里
/// 有无散落文件无关（W75 的仓库范围纪律）。这同时让「未跟踪的探针文件」不参与覆盖集。
export function resolveSite(site, tracked) {
  const matcher = markerMatcher(site);
  return [...tracked].filter(filePath => matcher.matches(filePath)).sort();
}

/// 站点表自身的一致性。与 `shapeErrors()` 同一条纪律：挡的是**未来某次合法编辑**
/// 造成的**静默失去覆盖**，而不是当前的错。
export function siteShapeErrors(sites = TEST_REGISTRATION_SITES) {
  const errors = [];
  const seenIds = new Set();
  const seenHostMarker = new Set();
  for (const site of sites) {
    if (typeof site.id !== 'string' || site.id.length === 0) {
      errors.push('a registration site has no id');
      continue;
    }
    if (seenIds.has(site.id)) {
      errors.push(`registration site id ${site.id} is declared twice`);
    }
    seenIds.add(site.id);

    if (typeof site.host !== 'string' || site.host.length === 0) {
      errors.push(`site ${site.id} has no host — a site with no file cannot be verified`);
    }
    if (typeof site.marker !== 'string' || site.marker.length === 0) {
      errors.push(`site ${site.id} has no marker — a declared site with nothing written is a lie`);
    }
    if (!TEST_TIERS.includes(site.tier)) {
      errors.push(
        `site ${site.id} has tier ${JSON.stringify(site.tier)} which is not one of ${TEST_TIERS.join(', ')}`,
      );
    }
    const key = `${site.host}\u0000${site.marker}`;
    if (seenHostMarker.has(key)) {
      errors.push(`site ${site.id} duplicates the host+marker of another site — one is redundant`);
    }
    seenHostMarker.add(key);
  }
  return errors;
}

/// **扩展名层的闭世界**：每个**看起来像测试**的在册文件，其扩展名 MUST ∈ `TEST_EXTENSIONS`。
/// 一个新扩展名（`foo.test.ts`）若不被认，它既不在覆盖集里、也不在台账里 ⇒
/// 上一层的闭世界对它**看不见**。这条断言把它挡在门外并指名。
export function extensionErrors(tracked, extensions = TEST_EXTENSIONS) {
  const errors = [];
  for (const filePath of tracked) {
    if (!looksLikeTest(filePath)) continue;
    if (!extensions.some(extension => String(filePath).endsWith(extension))) {
      const tail = String(filePath).slice(String(filePath).lastIndexOf('.'));
      errors.push(
        `test-like file ${filePath} has extension ${tail}, which is not one of ${extensions.join(', ')} — a test extension nobody scans is a blind spot`,
      );
    }
  }
  return errors;
}

/// 主判据。入参显式注入，**缺任一项即报错**（与 `validatePendingDecisions` 同纪律——
/// 一条会因入参缺失而恒真的断言比没有断言更坏）。
///
/// `deps`：
/// - `tracked`：**已跟踪**路径的 `Set`（来自 `readTrackedPaths`）；
/// - `sites`：站点表（默认 `TEST_REGISTRATION_SITES`）；
/// - `unreachable`：未覆盖台账（默认 `LOCALLY_UNREACHABLE`）；
/// - `readHost(host) => string | null`：读 host 文件正文（null 表示读不到）。
export function reachabilityErrors(deps = {}) {
  const { tracked, readHost } = deps;
  const sites = deps.sites ?? TEST_REGISTRATION_SITES;
  const unreachable = deps.unreachable ?? LOCALLY_UNREACHABLE;
  const errors = [];

  if (!(tracked instanceof Set)) {
    errors.push('reachability requires the tracked-path set and it was not provided');
  }
  if (typeof readHost !== 'function') {
    errors.push('reachability requires a readHost(host) function and it was not provided');
  }
  if (errors.length > 0) return errors;

  errors.push(...siteShapeErrors(sites));
  errors.push(...extensionErrors(tracked));

  const covered = new Set();
  for (const site of sites) {
    const text = readHost(site.host);
    if (text === null || text === undefined) {
      errors.push(`site ${site.id} names host ${site.host}, which is not readable in the repository`);
      continue;
    }
    // **假契约**：声明的 marker 必须**逐字**落在 host 里。
    // 「名字在表里」≠「线上真的有它」——只登记不落地的站点必须红。
    if (!String(text).includes(site.marker)) {
      errors.push(
        `site ${site.id} declares marker ${JSON.stringify(site.marker)} but host ${site.host} does not contain it verbatim — a declared site with nothing written is a lie`,
      );
      continue;
    }
    const hits = resolveSite(site, tracked);
    // **非空洞**：一个匹配不到任何**在册**文件的站点，是一条空声明。
    if (hits.length === 0) {
      errors.push(
        `site ${site.id} matches no tracked file (marker ${JSON.stringify(site.marker)}) — a site that covers nothing is a lie`,
      );
      continue;
    }
    for (const filePath of hits) covered.add(filePath);
  }

  // **闭世界**：每个在册测试文件 MUST ∈ 覆盖集 ∪ 台账。
  const ledger = new Map();
  for (const entry of unreachable) {
    if (!entry || typeof entry.path !== 'string' || typeof entry.reason !== 'string'
      || entry.reason.trim().length === 0) {
      errors.push('every unreachable-ledger entry needs a path and a non-empty reason');
      continue;
    }
    ledger.set(entry.path, entry.reason);
  }

  for (const filePath of tracked) {
    if (!isTestFile(filePath)) continue;
    if (!covered.has(filePath) && !ledger.has(filePath)) {
      errors.push(
        `test file ${filePath} is tracked but no registration site covers it and it is not in the unreachable ledger — it would never run`,
      );
    }
  }

  // **两集不相交**：一个文件名只属于一个类别。台账里的条目若**已经可达**，
  // 那它是一条**过期豁免**（台账不是垃圾桶）——必须红。
  for (const filePath of ledger.keys()) {
    if (covered.has(filePath)) {
      errors.push(
        `unreachable ledger lists ${filePath}, but a registration site already covers it — a stale exemption hides the site that now runs it`,
      );
    }
    if (!isTestFile(filePath)) {
      errors.push(
        `unreachable ledger lists ${filePath}, which is not a recognized test file — the ledger must not hold non-tests`,
      );
    } else if (!tracked.has(filePath)) {
      errors.push(
        `unreachable ledger lists ${filePath}, which is not tracked — the ledger must describe repository content`,
      );
    }
  }

  // **覆盖集非空**：与「台账为空时两集不相交退化成恒真」成对。若站点表被清空或全部解析失败，
  // 上面的闭世界会因台账为空而**全部静默通过**——这条是本轮「全不覆盖也算绿」的唯一守卫。
  if (covered.size === 0) {
    errors.push(
      'no registration site covers any tracked file — an empty coverage set makes the closed world vacuously true',
    );
  }

  return errors;
}
