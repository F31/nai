/// OpenAPI **文档集**与每份文档的**操作集闭世界**。
///
/// 这个模块存在的理由（W85，ADR 0063）：`apiDocument()` 的两道逐操作检查（权限声明、实现状态）
/// 都用了一张**手写的方法清单**：
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
///
/// 补法与 W81／W83／W84 同形：
/// 1. **触发条件 MUST 只由「文档里存在这个方法键」决定**（W83），MUST NOT 由手写清单决定——
///    判据沿 `HTTP_METHODS` 逐个键取操作，于是「新方法」与「老方法」走同一条路径。
/// 2. **方法键集 MUST 封闭**（W81／W84）：path item 的每个键 MUST ∈ `HTTP_METHODS` ∪
///    `PATH_ITEM_FIELDS`（`x-` 扩展前缀按 OpenAPI 规定放行）；
/// 3. **相邻层不相交**（W82）：`HTTP_METHODS` 与 `PATH_ITEM_FIELDS` MUST 无交集；
/// 4. **声明无孤儿**（W75「已声明 ≠ 存在」）：声明表的每一项 MUST 指回文档里真实存在、
///    且确实带 `IMPLEMENTED_STATUS` 的操作；
/// 5. **非空洞**：文档 MUST 至少暴露一个操作。
///
/// **W86（ADR 0064）补的是同一句话的上一层**：上面整段说完的是「**哪些操作**被校验」，
/// 而「**哪些文档**被校验」仍然是一张手写清单——`apiDocument()` 把路径硬编码成
/// `schemas/openapi/control.yaml`。清单的补集就是盲区：同目录的 `remote-knowledge.yaml`
/// **本地门控从不读它**（唯一读它的 `generate.mjs` 只在 CI 的 `cargo xtask schema --check` 里跑，
/// 而 `schema --check` **不在本机门控闭包里**——它的工具链前提是 `protoc`，超出 portable 门控
/// 「只需 Rust 工具链」的既有不变式；成因见 W108 / ADR 0086），而它对「操作不声明权限」**放行**。实测：往 remote 文档加一个
/// 无权限的 `patch`，`SwaggerParser` 通过、本机门控全绿。
///
/// 于是补法仍是闭世界，只是对象换成了**文档集**：
/// 6. **文档集由已跟踪路径派生**（W75／W84），MUST NOT 手写「有哪些文档」；
/// 7. **逐文档声明 `OPENAPI_DOCUMENTS` 是单一来源**（状态 + 已实现表），逐字写死；
/// 8. **判据逐文档参数化，`declared` 是必需入参**——缺参即具名失败，MUST NOT 退回
///    另一份文档的默认值（用错对象的断言比没有断言更坏）。

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

/// 仓库里 OpenAPI 描述所在的目录与其**被识别的扩展名**。
///
/// 文档集从**已跟踪路径**（`.git/index`）派生，MUST NOT 手写一份「有哪些文档」——
/// 手写的那一份的补集就是盲区，而盲区里的文档**不会被任何判据看见**。
/// 与 W84 同一条理由：用索引而不是 `readdir`，让「仓库里有哪些契约」的输入就是**仓库**，
/// 于是工作区里未跟踪的散落文件不会参与、也不会掩盖覆盖关系。
export const OPENAPI_DIR = 'schemas/openapi';

/// 被视为 OpenAPI 描述的扩展名。逐字写死（W84 的「扩展名层闭世界」同形）：
/// 一个不在表里的扩展名会让文档**从文档集里消失**，而且没有任何信号。
export const OPENAPI_EXTENSIONS = ['.yaml', '.yml'];

/// **逐文档声明**：每份 OpenAPI 描述的（顶层实现状态期望值、已实现操作表）。
///
/// 这是本模块的**单一来源**，逐字写死。键 = 仓库相对路径。两件事都从执行体搬到了这里：
/// - `status`：原先硬编码在 `apiDocument()` 里的字符串比较（`'partial-loopback-engineering'`）；
/// - `implemented`：原先一张全局的 `IMPLEMENTED_OPERATIONS`——但「已实现的是哪些操作」是
///   **每一份文档各自的属性**。`remote-knowledge.yaml` 自述 `client-only`，它的已实现表**为空**，
///   把 `control.yaml` 的八条套上去只会得到八条孤儿声明。
///
/// 每份文档的已实现表被 `operationErrors` **反查**——表里的一条改名/删端点会变成一条
/// **只有声明、没有对象**的契约，且零信号。
export const OPENAPI_DOCUMENTS = {
  'schemas/openapi/control.yaml': {
    status: 'partial-loopback-engineering',
    implemented: [
      'get /health',
      'get /runtime/status',
      'get /sessions/{id}',
      'post /sessions/{id}/mute',
      'post /sessions/{id}/takeover',
      'get /approvals',
      'post /approvals/{id}/approve',
      'post /approvals/{id}/reject',
      'post /upgrades/plan',
    ],
  },
  'schemas/openapi/remote-knowledge.yaml': {
    status: 'client-only',
    implemented: [],
  },
};

/// 已实现操作**应当**携带的实现状态值。
export const IMPLEMENTED_STATUS = 'implemented-loopback';

/// **仓库里被识别的 OpenAPI 描述**（已跟踪路径 ∩ `OPENAPI_DIR` ∩ `OPENAPI_EXTENSIONS`）。
///
/// 纯函数：只看路径名，不看内容——闭世界 MUST 只看名字（W81）。
/// 入参不是 `Set` 时返回 `null`：**「没给输入」与「输入是空集」是两件事**，
/// 前者 MUST 在 `documentSetErrors` 里具名失败，MUST NOT 被当成「仓库里没有文档」。
export function openapiDocumentPaths(tracked) {
  if (!(tracked instanceof Set)) return null;
  const prefix = `${OPENAPI_DIR}/`;
  return [...tracked]
    .filter(filePath => filePath.startsWith(prefix))
    .filter(filePath => OPENAPI_EXTENSIONS.some(extension => filePath.endsWith(extension)))
    .sort();
}

/// **文档集闭世界** + 逐文档声明的形状。与 `shapeErrors()`（W80）／`siteShapeErrors()`（W84）
/// 同一条纪律：挡的是**未来某次合法编辑**造成的**静默失去覆盖**，而不是当前的错。
///
/// 闭世界是**双向**的，两条各自报具名：
/// - 在册描述 MUST **全部被声明**（新增一份文档而忘了声明 ⇒ 它无人校验）；
/// - 声明 MUST **全部对应在册描述**（文档改名/删除而声明留着 ⇒ 一条没有对象的覆盖）。
/// 只有两条一起才等于「集合相等」。
///
/// 非空洞：文档集非空；且**至少一份**文档声明了非空已实现表——否则「声明无孤儿」这一半
/// 在所有文档上**空转**（W84 的「空台账使两集不相交退化成恒真」同形）。
export function documentSetErrors(tracked, documents = OPENAPI_DOCUMENTS) {
  const errors = [];
  const onDisk = openapiDocumentPaths(tracked);
  if (onDisk === null) {
    errors.push(
      'the document-set closure requires the tracked-path set and it was not provided — falling back to "no documents" would make every description escape',
    );
    return errors;
  }

  const declared = Object.keys(documents);
  if (declared.length === 0) {
    errors.push(
      'OPENAPI_DOCUMENTS is empty — every tracked description would be checked by nothing',
    );
  }

  for (const docPath of onDisk) {
    if (!Object.hasOwn(documents, docPath)) {
      errors.push(
        `the repository tracks the OpenAPI description ${docPath}, but OPENAPI_DOCUMENTS does not declare it — a description nobody declares is a description nobody checks`,
      );
    }
  }
  for (const docPath of declared) {
    if (!onDisk.includes(docPath)) {
      errors.push(
        `OPENAPI_DOCUMENTS declares ${docPath}, which is not a tracked OpenAPI description under ${OPENAPI_DIR}/ — a declaration with no document`,
      );
    }
  }

  let anyImplemented = false;
  for (const [docPath, entry] of Object.entries(documents)) {
    if (!entry || typeof entry !== 'object') {
      errors.push(`the declaration for ${docPath} is not an object, so its document cannot be judged`);
      continue;
    }
    if (typeof entry.status !== 'string' || entry.status.trim().length === 0) {
      errors.push(
        `the declaration for ${docPath} has no status — the document's top-level implementation status would go unchecked`,
      );
    }
    if (!Array.isArray(entry.implemented)) {
      errors.push(`the declaration for ${docPath} has no implemented list`);
      continue;
    }
    const seen = new Set();
    for (const item of entry.implemented) {
      if (typeof item !== 'string' || !/^[a-z]+ \//.test(item)) {
        errors.push(
          `the declaration for ${docPath} lists ${JSON.stringify(item)}, which is not of the form "<method> <path>"`,
        );
        continue;
      }
      const [method] = item.split(' ');
      if (!HTTP_METHODS.includes(method)) {
        errors.push(
          `the declaration for ${docPath} lists ${item}, whose method ${method} is not a recognised HTTP method — the closed world would never see it`,
        );
      }
      if (seen.has(item)) {
        errors.push(`the declaration for ${docPath} lists ${item} twice`);
      }
      seen.add(item);
    }
    if (entry.implemented.length > 0) anyImplemented = true;
  }

  if (declared.length > 0 && !anyImplemented) {
    errors.push(
      'no document declares an implemented operation — the orphan half of the closure would be vacuously true on every document',
    );
  }

  return errors;
}

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

/// **两张键集**自身的一致性：不相交、且都非空。
///
/// 逐文档的**声明形状**（合法方法、无重复、非空）住在 `documentSetErrors` 里——
/// 那里才有「这是哪一份文档的声明」这个上下文，把两者混在一个函数里会让错误少一半信息。
export function operationShapeErrors(methods = HTTP_METHODS, fields = PATH_ITEM_FIELDS) {
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
  return errors;
}

/// 主判据。入参是**文档对象**（已解析的 OpenAPI）与该文档**自己的**已实现声明；出参是具名错误列表。
///
/// `declared` 是**必需入参**：缺它就退回「某个模块级默认值」，那等于**用另一份文档的声明去判**
/// 这份文档——判据的输入不再等于被断言的对象。因此缺参／类型不对都**具名失败**，
/// MUST NOT 静默降级（与 `resolveSite`／`reconcile`／`validatePendingDecisions` 同纪律）。
///
/// 文档缺失或结构不可解析时同样**失败**，MUST NOT 静默通过——一条不可判定的断言比没有断言更坏。
export function operationErrors(document, declared) {
  const errors = [];
  errors.push(...operationShapeErrors());

  if (!Array.isArray(declared)) {
    errors.push(
      'operationErrors requires the document\'s own implemented-declaration array and it was not provided — judging with another document\'s declaration would make the input differ from the thing asserted',
    );
    return errors;
  }

  const paths = document?.paths;
  if (!document || typeof document !== 'object' || !paths || typeof paths !== 'object') {
    errors.push(
      'the API description exposes no readable paths — every per-operation check would be vacuously true',
    );
    return errors;
  }

  const declaredSet = new Set(declared);
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
      const expected = declaredSet.has(key) ? IMPLEMENTED_STATUS : undefined;
      if (operation['x-implementation-status'] !== expected) {
        errors.push(
          expected === undefined
            ? `${key} is not declared implemented but claims x-implementation-status ` +
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
  for (const entry of declaredSet) {
    if (!present.has(entry)) {
      errors.push(
        `the implementation declaration names ${entry}, but the API description has no such operation — a declaration with no object`,
      );
    }
  }

  return errors;
}
