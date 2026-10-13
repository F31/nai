/// OpenAPI 描述的**操作集闭世界**。
///
/// 这个模块存在的理由：`apiDocument()` 的两道逐操作检查（权限声明、实现状态）都用了一张
/// **手写的方法清单**：
///
/// ```js
/// for (const method of ['get', 'post', 'put', 'delete']) { … }
/// ```
///
/// 手写清单能挡住「清单内的方法被错写」，挡不住「清单外的方法被**新增**」——后者的补集恰恰是
/// 清单不覆盖的那一块。`patch`／`head`／`options`／`trace` 都是 OpenAPI 3.0 的合法方法键，
/// 全部落在清单之外。
///
/// 实测（W85 探针）：往 `paths` 里加一个**无** `x-required-permission`、**无**
/// `x-implementation-status` 的 `patch` 操作 ⇒ 判定返回**空错误**；同位置换成 `get` ⇒
/// 立刻具名报错。`SwaggerParser` 两种情况都放行（它管结构合法性，不管覆盖）。
/// ⇒ 一整类操作可以从契约门口走进来而无人过问。
///
/// 补法与 W81／W83／W84 同形：
/// 1. **触发条件 MUST 只由「文档里存在这个方法键」决定**（W83），MUST NOT 由手写清单决定——
///    判据沿 `HTTP_METHODS` 逐个键取操作，于是「新方法」与「老方法」走同一条路径。
/// 2. **方法键集 MUST 封闭**（W81／W84）：path item 的每个键 MUST ∈ `HTTP_METHODS` ∪
///    `PATH_ITEM_FIELDS`（`x-` 扩展前缀按 OpenAPI 规定放行）；
/// 3. **相邻层不相交**（W82）：`HTTP_METHODS` 与 `PATH_ITEM_FIELDS` MUST 无交集；
/// 4. **声明无孤儿**（W75「已声明 ≠ 存在」）：`IMPLEMENTED_OPERATIONS` 的每一项 MUST 指回
///    文档里真实存在、且确实带 `IMPLEMENTED_STATUS` 的操作；
/// 5. **非空洞**：文档 MUST 至少暴露一个操作。

/// **被识别的 HTTP 方法键**。逐字写死，MUST NOT 由「文档里现有方法」派生——
/// 派生出的「现有方法 == 清单」是恒真式，那样闭世界就变成了复述事实。
///
/// 这份清单取自 **OpenAPI 3.0 规范**允许的方法键全表（八个），不是「仓库当前用到的那些」。
/// 仓库此刻只用了 `get`／`post`／`put`／`delete`，但清单 MUST NOT 随之收缩：
/// 收缩后 `patch` 这类方法就会重新落进补集。
export const HTTP_METHODS = ['get', 'put', 'post', 'delete', 'options', 'head', 'patch', 'trace'];

/// **path item 上的非方法字段**。逐字写死，取自 OpenAPI 3.0 的 Path Item Object：
/// `$ref` / `summary` / `description` / `servers` / `parameters`。
///
/// 与 `HTTP_METHODS` 合起来构成 path item 层的闭集：一个键若两者都不是，就既不是操作、
/// 也不是标准字段——它是笔误或未声明的扩展，MUST 被具名拒绝。
export const PATH_ITEM_FIELDS = ['$ref', 'summary', 'description', 'servers', 'parameters'];

/// OpenAPI 规范扩展的前缀（规范规定规范扩展 MUST 以 `x-` 开头）。
///
/// 放行它**不重开洞**：扩展**不是操作**，不参与「逐操作检查」的语义；
/// 而一个想冒充操作的名字（`gets`／`patch2`）不以 `x-` 开头，仍被具名拒绝。
export const EXTENSION_PREFIX = 'x-';

/// **声明为已实现的操作**。逐字写死，`<method> <path>` 形式。
///
/// 这是本模块的**单一来源**：`validation.mjs` 的 `apiDocument()` 从这里取，不再自己写一份。
/// 表本身被 `operationErrors` **反查**——每一项 MUST 在文档里真实存在、且状态为
/// `IMPLEMENTED_STATUS`。没有这一条，一条改名或删端点的改动会留下一条**只有声明、没有对象**
/// 的契约，且零信号。
///
/// 实测当前文档**零孤儿**；但本轮之前没有任何守卫会拦住第一个孤儿。
export const IMPLEMENTED_OPERATIONS = [
  'get /health',
  'get /runtime/status',
  'get /sessions/{id}',
  'post /sessions/{id}/mute',
  'post /sessions/{id}/takeover',
  'get /approvals',
  'post /approvals/{id}/approve',
  'post /approvals/{id}/reject',
];

/// `IMPLEMENTED_OPERATIONS` 里每条操作**应当**携带的实现状态值。
export const IMPLEMENTED_STATUS = 'implemented-loopback';

/// 文档里**被识别**的全部操作，`{method, path}` 列表。顺序按文档的键序。
///
/// 「被识别」= 键 ∈ `HTTP_METHODS`。这是**闭世界**的正面：不在 `HTTP_METHODS` 里的键
/// 由 `operationErrors` 的另一半具名报出，而不是在这里被悄悄丢弃。
export function operations(document) {
  const out = [];
  const paths = document?.paths;
  if (!paths || typeof paths !== 'object') return out;
  for (const [path, item] of Object.entries(paths)) {
    if (!item || typeof item !== 'object') continue;
    for (const key of Object.keys(item)) {
      if (HTTP_METHODS.includes(key)) out.push({ method: key, path });
    }
  }
  return out;
}

/// 三张表自身的一致性。与 `shapeErrors()`（W80）/ `siteShapeErrors()`（W84）同一条纪律：
/// 挡的是**未来某次合法编辑**造成的**静默失去覆盖**，而不是当前的错。
export function operationShapeErrors(
  methods = HTTP_METHODS,
  fields = PATH_ITEM_FIELDS,
  declared = IMPLEMENTED_OPERATIONS,
) {
  const errors = [];
  const overlap = methods.filter(method => fields.includes(method));
  if (overlap.length > 0) {
    errors.push(
      `HTTP method keys and path-item fields overlap: ${overlap.join(', ')} — a key cannot be both, and the closed world would be decided twice`,
    );
  }
  if (methods.length === 0) {
    errors.push('HTTP_METHODS is empty — every operation key would fall outside the closed set');
  }
  if (fields.length === 0) {
    errors.push('PATH_ITEM_FIELDS is empty — every path item would be reported as unknown');
  }
  if (declared.length === 0) {
    errors.push('IMPLEMENTED_OPERATIONS is empty — the declaration would be vacuously satisfied');
  }
  const seen = new Set();
  for (const entry of declared) {
    if (typeof entry !== 'string' || !/^[a-z]+ \//.test(entry)) {
      errors.push(
        `implemented operation ${JSON.stringify(entry)} is not of the form "<method> <path>"`,
      );
      continue;
    }
    const [method] = entry.split(' ');
    if (!methods.includes(method)) {
      errors.push(
        `implemented operation ${entry} names method ${method}, which is not a recognised HTTP method — the closed world would never see it`,
      );
    }
    if (seen.has(entry)) {
      errors.push(`implemented operation ${entry} is declared twice`);
    }
    seen.add(entry);
  }
  return errors;
}

/// 主判据。入参是**文档对象**（已解析的 OpenAPI），出参是具名错误列表；空列表 = 一致。
///
/// 文档缺失或结构不可解析时**失败**，MUST NOT 静默通过——一条不可判定的断言比没有断言更坏
/// （与 `reconcile`／`reachabilityErrors` 同纪律）。
export function operationErrors(document) {
  const errors = [];
  errors.push(...operationShapeErrors());

  const paths = document?.paths;
  if (!document || typeof document !== 'object' || !paths || typeof paths !== 'object') {
    errors.push(
      'the API description exposes no readable paths — every per-operation check would be vacuously true',
    );
    return errors;
  }

  const declared = new Set(IMPLEMENTED_OPERATIONS);
  const present = new Set();

  for (const [path, item] of Object.entries(paths)) {
    if (!item || typeof item !== 'object') {
      errors.push(`path ${path} is not an object, so its operations cannot be read`);
      continue;
    }
    // 键集闭世界：方法、标准字段、或 `x-` 扩展，三选一。
    for (const key of Object.keys(item)) {
      if (HTTP_METHODS.includes(key) || PATH_ITEM_FIELDS.includes(key)) continue;
      if (key.startsWith(EXTENSION_PREFIX)) continue;
      errors.push(
        `path ${path} has key ${JSON.stringify(key)}, which is neither a recognised HTTP method nor a path-item field — it would carry an operation nobody checks`,
      );
    }
    // 触发条件**只由「存在这个方法键」决定**（W83）：沿 HTTP_METHODS 的全表取操作，
    // 于是 `patch` 与 `get` 走同一条路径。
    for (const method of HTTP_METHODS) {
      const operation = item[method];
      if (!operation || typeof operation !== 'object') continue;
      const key = `${method} ${path}`;
      present.add(key);
      if (!operation['x-required-permission']) {
        errors.push(`${key} declares no x-required-permission`);
      }
      const expected = declared.has(key) ? IMPLEMENTED_STATUS : undefined;
      if (operation['x-implementation-status'] !== expected) {
        errors.push(
          expected === undefined
            ? `${key} is not in IMPLEMENTED_OPERATIONS but claims x-implementation-status ` +
              `${JSON.stringify(operation['x-implementation-status'] ?? null)}`
            : `${key} is declared implemented but its x-implementation-status is ` +
              `${JSON.stringify(operation['x-implementation-status'] ?? null)}, expected ` +
              `${JSON.stringify(IMPLEMENTED_STATUS)}`,
        );
      }
    }
  }

  // 非空洞：与 W84 的「覆盖集非空」同形。文档不暴露任何操作时，上面的逐操作检查
  // 全部为真但**空转**。
  if (present.size === 0) {
    errors.push(
      'the API description exposes no operations — the per-operation checks are vacuously true',
    );
  }

  // 声明无孤儿（W75：已声明 ≠ 存在）。
  for (const entry of declared) {
    if (!present.has(entry)) {
      errors.push(
        `IMPLEMENTED_OPERATIONS declares ${entry}, but the API description has no such operation — a declaration with no object`,
      );
    }
  }

  return errors;
}
