/// **作业面闭世界**（W144，ADR 0118）。
///
/// 问题形状：`control.yaml` 有 15 个操作声明 `202 + JobAccepted`，而唯一读作业状态的
/// `GET /jobs/{id}` 只读 `upgrade_journal` 一张表——契约自己承认这件事：
///
/// ```yaml
/// kind: { enum: [upgrade], description: '... only upgrade jobs have a journal today.' }
/// ```
///
/// 于是其余 14 条「铸了一个 `job_id`，而那个 id 指不到任何状态」。收口这件事**不是**
/// 一次性实现 15 套状态机——那是 15 次数据库迁移，而且每条路由的状态形状至今没有人设计过。
/// 本轮收的是**声明面**：
///
/// 1. 每一条 `202 + JobAccepted` MUST 在册（`tests/job-surface.json`），MUST NOT 有一条能悄悄多出来；
/// 2. 在册项 MUST 二者**恰有其一**——`kind`（真能被 `/jobs/{id}` 轮询）或具名的
///    `blockedBy`（今天不铸 `job_id` 的理由）。
///
/// 第二条是关键：它把「我也不知道该怎么办」这个状态**从沉默变成一个必须写的值**。
/// 加一条新的 202 而不写这两样之一 ⇒ 具名失败，而不是又一次静默的伪造可观测面。
///
/// 为什么不是「全部改掉」：把 202 换成同步 `200` 要么伪造成功（操作根本没做），
/// 要么让整族端点变成 503（能力真的缺，但那是另一件事）；删掉 `JobAccepted` 会让
/// `JobView.kind` 的共享 id 空间声明**失去对端**，而 `upgrades/execute` 将来仍要用它。
/// 两条都写进 ADR 0118 的「被否决方案」。
///
/// 判据分两层，各自有独立的输入域，MUST NOT 合并：
/// - `jobSurfaceErrors`：**契约侧**（文档对象里的 202 与 schema）；
/// - `jobSourceErrors`：**仓库侧**（声明指向的 journal/reader/**writer**/拒绝点/安装器是否真的在仓库里）。
///
/// **W146（ADR 0120）补的是「谁写这行」和「谁推进它」**：前一轮把「分类 ⇔ 路由器怎么处理」
/// 接上了，但那个分类的另一半——`kind: upgrade` 说「这个 id 能被 `/jobs/{id}` 轮询」——
/// 仍然只有两个锚点作保（表在、有人读）。实测：`upgrade_journal` 的唯一写入点在
/// `record_upgrade`，而推进事务的两个执行入口**没有任何 tests 目录之外的调用点**，
/// 唯一的 `UpgradeHost` 实现者是演习替身。两端共同指向同一个缺席。
/// ⇒ 本轮不实现安装器（那是真值缺口），只把「写入点唯一」「安装器名册封闭」
/// 「能轮询的作业面 ⇔ 生产安装器」这三件事变成可判的。

import { HTTP_METHODS, OPENAPI_DOCUMENTS } from './openapi-ops.mjs';

/// 「已接受，尚未执行」的状态键。逐字写死：换成 `'201'` 或 `'200'` 就不再是作业面，
/// 那时这条判据应当**看不见**那些操作，而不是把它们当成作业。
export const ACCEPTED_STATUS_KEY = '202';

/// 202 响应体 MUST 引用的 schema。
///
/// 它 MUST 是 `$ref` 字面、MUST NOT 是内联对象：内联之后「同一个 `JobAccepted`」这件事
/// 就没有对象可比对了，15 条路由可以各自长成一个不同的 `{job_id, status}` 而不被任何判据发现。
export const JOB_ACCEPTED_SCHEMA = '#/components/schemas/JobAccepted';

/// 共享作业 id 空间的判别 schema。`kind` 的枚举是「有哪些状态来源」的契约侧声明。
export const JOB_VIEW_SCHEMA = '#/components/schemas/JobView';

/// `JobAccepted.status` 唯一允许的取值。
///
/// 钉死它，是因为**它是本轮论点的对端**：`accepted` 只说「收到了」，不说「会执行」。
/// 一旦这里多出 `queued`／`running` 之类的值，一条没有 journal 的路由就能靠响应体
/// 自称「进行中」——那是比铸一个指不到的 id 更硬的谎，因为客户端会照着它去等。
export const ACCEPTED_STATUS_ONLY = 'accepted';

/// **不铸 `job_id` 的理由**（闭集）。
///
/// 两个值 MUST NOT 混用：它们的**失败形状**不同，混同会让「做不到」与「没有这条」在
/// 客户端不可区分（ADR 0113 的同一条纪律）。
///
/// 每一项都带一个 `anchor`：声明指向仓库里的一段**逐字文本**。没有对端的声明就是一条
/// 注释，注释不会变红。锚点选的是「这句话一旦改掉，这个声明就不再成立」的那一句。
export const JOB_BLOCKERS = {
  no_handler: {
    why:
      '路由表里没有这条分支：请求落到兜底的 404 CapabilityAbsent。它不是「暂时缺依赖」，' +
      '而是这个操作在本进程里根本不存在——所以既不能答 202，也不能答 503。',
    anchor: {
      file: 'crates/nai-control-api/src/lib.rs',
      text: 'control operation not implemented',
    },
  },
  refuses_without_installer: {
    why:
      '有专用分支，但缺平台安装器（`UpgradeHost` 的生产实现）：整条路在第一步之前就 503 ' +
      'CapabilityNotReady，并且源码里明确写了不铸作业 id。换成 202 会把「做不到」说成「进行中」。',
    anchor: {
      file: 'crates/nai-control-api/src/lib.rs',
      text: 'no job id is minted',
    },
  },
};

/// **写入侧的扫描域**：状态一行一旦写进去，`/jobs/{id}` 才可能有 200。
///
/// 扩展名逐字写死（与 W84 的「扩展名层闭世界」同形）：一个不在表里的扩展名会让
/// **写入点从扫描域里消失**，而信号是「找不到写入点」——那条错误会把「没扫到」
/// 说成「不存在」。`.sql` 必须在内：迁移文件里也能写一行。
export const WRITE_SCAN_EXTENSIONS = ['.rs', '.sql'];

/// `impl UpgradeHost for <Name>` 的逐字形态。取 Rust 语法片段而不是「名字出现过」——
/// 后者在注释、`use` 与类型参数里都会命中，于是名册与扫描永远对不上，而且看起来像名册错了。
const HOST_IMPL = /impl\s+UpgradeHost\s+for\s+([A-Za-z_][A-Za-z0-9_]*)/g;

/// **状态来源**（闭集）：`JobView.kind` 的每个取值 MUST 在这里有一份 journal 与一个 reader。
///
/// 两个锚点各管一半，且都不由构造保证：
/// - `journal` 指向**这张表本身**（它不存在 ⇒ 状态无处可读）；
/// - `reader` 指向**把该 kind 写出来的那一行**（它改名/搬走 ⇒ 表还在但没有读者能报出这个 kind）。
///
/// reader 的锚点刻意选 `"kind": "upgrade"` 而不是 `"upgrade"`：同一个文件里
/// `upgrade_refusal` 的类别名也是 `"upgrade"`，只锚单词的话两条路会被同一处命中，
/// 于是「reader 不再报告这个 kind」这件事永远不会变红。
///
/// **`writer`（W146）补的是第三端**：前两个锚点只说「表在」「有人读」，都没说「谁写」。
/// 一个只有读者、没有写入者的表，`/jobs/{id}` 的 404 分支可达、200 分支不可达——
/// 而这正是今天 `upgrade` 的真实形状（见 ADR 0120）。`statement` 是那次写入的**逐字语句**，
/// 它让「写入点唯一」可以被判（见 `jobSourceErrors`），而不是靠「我知道只有一处」。
export const JOB_KINDS = {
  upgrade: {
    journal: {
      file: 'crates/nai-storage/src/upgrade.rs',
      text: 'pub struct UpgradeRecord',
    },
    reader: {
      file: 'crates/nai-control-api/src/lib.rs',
      text: '"kind": "upgrade"',
    },
    writer: {
      file: 'crates/nai-storage/src/upgrade.rs',
      // 锚**调用形态**而不是声明前缀：`pub fn record_upgrade` 是前缀形态，改名成
      // `record_upgrade_v2` 仍然包含它 ⇒ 锚点照旧绿（W146 注入实测：I2 首轮没红）。
      // 带上前缀 `(` 与 `&mut self` 之后，「换名字」「换接收者」都会让它失去对端。
      text: 'record_upgrade(&mut self',
      statement: 'INSERT INTO upgrade_journal',
    },
  },
};

/// **生产者名册**（闭集）：`UpgradeHost` 的每个实现者 MUST 在这里有一行，且 MUST 交代
/// 它是不是**生产**实现。
///
/// 为什么需要它：`job id` 的两端挂在同一个 trait 上——`upgrades/execute` 要它才能推进事务，
/// `upgrade_journal` 的行也只有它能让 `UpgradeTransaction` 写出来。今天唯一的实现者叫
/// `RehearsalHost`（演习替身），于是「生产上没有安装器」这句话是**真**的，但此前它只是
/// 一句散文：去掉这个实现者不会有任何判据变红，加一个生产实现者也不会有。
///
/// `production` MUST 是布尔，MUST NOT 用「名字里没有 Rehearsal」来推断——那种推断在
/// 第一个叫 `PlatformHost` 的演习替身出现时就错了，而且错得没有信号。
export const JOB_HOSTS = {
  RehearsalHost: {
    file: 'crates/nai-upgrade-core/src/rehearsal.rs',
    production: false,
    why:
      '演习替身：整份事实由 `demo_facts` 供给、改动物理世界只在内存里记账，' +
      '不落盘、不换 manifest。它能证明「事务编排对」，不能证明「这台机器真的升级了」。',
  },
};


/// **登记表的落点**——两侧共读的**单一枢纽**。
///
/// 15 条分类不写在本模块里，而是放在 `tests/job-surface.json`。理由是**两侧必须读同一份字节**：
/// - 契约侧（本模块 + `schema.test.mjs`）判「登记集 ⇔ 契约的 202 集」；
/// - 执行侧（`crates/nai-control-api/src/tests.rs`）读同一份文件，把每条 `(method, path)` 打进
///   真实路由器，判「分类 ⇔ 路由器**怎么处理**它」。
///
/// 缺口原来就在这里：两侧各自全绿而数据分叉，等于「声明被相信、行为没人问」（W145 的起点）。
/// ⇒ `consumer.text` 逐字钉住执行侧读的是哪一份文件——指向别处就等于两侧在判不同的东西。
/// 本模块**不读盘**（只判定），由调用方读进来传给它，判定本身才能被合成输入驱动。
export const JOB_REGISTRY = {
  path: 'tests/job-surface.json',
  consumer: {
    file: 'crates/nai-control-api/src/tests.rs',
    text: 'include_str!("../../../tests/job-surface.json")',
  },
};

/// 文档里**接受为作业**的操作，`{key, accepted}` 列表。
///
/// 触发条件 MUST 只由「`responses` 里有 `202` 这个键」决定——沿 `HTTP_METHODS` 全表逐个取，
/// 于是新增一个 `patch` 操作与既有 `post` 走同一条路径（W83 的纪律）。
///
/// **调用方 MUST 传 `SwaggerParser.parse` 的结果，MUST NOT 传 `validate` 的结果**：
/// `validate` 会把 `$ref` 内联，于是「202 的响应体是不是同一个 `JobAccepted`」这件事
/// 在入参里已经不可判定（实测：`validate` 口径下 `202_with_JobAccepted` 量出 `0`，
/// `parse` 口径下量出 `15`）。
export function acceptingOperations(document) {
  const out = [];
  const paths = document?.paths;
  if (!paths || typeof paths !== 'object') return out;
  for (const [path, item] of Object.entries(paths)) {
    if (!item || typeof item !== 'object') continue;
    for (const method of HTTP_METHODS) {
      const operation = item[method];
      if (!operation || typeof operation !== 'object') continue;
      const responses = operation.responses;
      if (!responses || typeof responses !== 'object') continue;
      if (!Object.hasOwn(responses, ACCEPTED_STATUS_KEY)) continue;
      out.push({ key: `${method} ${path}`, accepted: responses[ACCEPTED_STATUS_KEY] });
    }
  }
  return out;
}

/// 取 `#/components/schemas/<name>` 形式的引用在文档里的定义。不是该形式时返回 `undefined`。
function referencedSchema(document, ref) {
  const prefix = '#/components/schemas/';
  if (typeof ref !== 'string' || !ref.startsWith(prefix)) return undefined;
  return document?.components?.schemas?.[ref.slice(prefix.length)];
}

/// **契约侧主判据**。出参是具名错误列表，空数组 = 通过。
///
/// `documents` 是 `{文档路径: 已解析文档}`，键集 MUST 与 `OPENAPI_DOCUMENTS` 双向相等：
/// 少一份 ⇒ 那份文档的 202 无人看；多一份 ⇒ 一份没被声明的文档在参与判定。
/// 缺参即具名失败（`null`／非对象），MUST NOT 退化成「没有文档」——
/// 把「没给输入」当成「输入是空集」会让下面每一条规则同时空转。
///
/// `surfaces` 是登记表（`tests/job-surface.json` 解析后的对象），**没有默认值**：
/// 兜一个默认值就等于「注入一份空表」也会被判成「登记表被清空」而不是「没给」。
export function jobSurfaceErrors(documents, surfaces) {
  const errors = [];

  if (!documents || typeof documents !== 'object') {
    errors.push(
      'the job-surface closure requires the parsed OpenAPI documents and they were not provided — treating that as "no documents" would make every rule below vacuously true',
    );
    return errors;
  }
  if (!surfaces || typeof surfaces !== 'object') {
    errors.push(
      'the job-surface closure requires the registry (tests/job-surface.json) and it was not provided — an absent registry and an empty one are different answers',
    );
    return errors;
  }

  const declaredDocs = Object.keys(OPENAPI_DOCUMENTS).sort();
  const givenDocs = Object.keys(documents).sort();
  for (const docPath of declaredDocs) {
    if (!Object.hasOwn(documents, docPath)) {
      errors.push(
        `${docPath} is a declared OpenAPI description but was not provided to the job-surface closure — its 202 operations would be judged by nothing`,
      );
    }
  }
  for (const docPath of givenDocs) {
    if (!Object.hasOwn(OPENAPI_DOCUMENTS, docPath)) {
      errors.push(
        `${docPath} was provided to the job-surface closure but OPENAPI_DOCUMENTS does not declare it — a document that is not declared is a document nobody knows the rules for`,
      );
    }
  }
  for (const docPath of declaredDocs) {
    if (!Object.hasOwn(surfaces, docPath)) {
      errors.push(
        `the job-surface registry does not declare ${docPath} — an undeclared document starts from an empty surface, and an empty surface accepts nothing, so the closure would read as "nothing to check"`,
      );
    }
  }
  for (const docPath of Object.keys(surfaces)) {
    if (!Object.hasOwn(OPENAPI_DOCUMENTS, docPath)) {
      errors.push(
        `the job-surface registry declares ${docPath}, which OPENAPI_DOCUMENTS does not know — a registry entry with no document`,
      );
    }
  }

  let anySurface = false;
  let sawJobAccepted = false;
  let sawJobView = false;

  for (const docPath of declaredDocs) {
    const document = documents[docPath];
    const entry = OPENAPI_DOCUMENTS[docPath];
    const declared = Array.isArray(entry?.implemented) ? entry.implemented : [];

    if (!document || typeof document !== 'object') {
      errors.push(`${docPath}: no parsed document was supplied, so its job surface cannot be decided`);
      continue;
    }
    const paths = document.paths;
    if (!paths || typeof paths !== 'object') {
      errors.push(
        `${docPath}: the description exposes no readable paths — its job-surface rules would be vacuously true`,
      );
      continue;
    }

    const registry = surfaces[docPath];
    if (!registry || typeof registry !== 'object' || Array.isArray(registry)) {
      errors.push(`${docPath}: the job-surface registry entry is not an object, so its surface cannot be read`);
      continue;
    }
    if (Object.keys(registry).length > 0) anySurface = true;

    const accepting = acceptingOperations(document);
    const acceptingKeys = new Set(accepting.map(row => row.key));

    for (const row of accepting) {
      // 规则 2：202 的响应体 MUST 是同一个 `JobAccepted` 的 `$ref`。
      const content = row.accepted && typeof row.accepted === 'object' ? row.accepted.content : undefined;
      const schema = content && typeof content === 'object' ? content['application/json']?.schema : undefined;
      if (!schema || typeof schema !== 'object') {
        errors.push(
          `${docPath}: ${row.key} answers ${ACCEPTED_STATUS_KEY} with no application/json body schema — a client cannot even learn the job id`,
        );
      } else if (schema.$ref !== JOB_ACCEPTED_SCHEMA) {
        errors.push(
          `${docPath}: ${row.key} answers ${ACCEPTED_STATUS_KEY} with ${
            schema.$ref === undefined ? 'an inline schema' : JSON.stringify(schema.$ref)
          } instead of ${JOB_ACCEPTED_SCHEMA} — inline bodies let each route grow its own job shape, and nothing could compare them`,
        );
      }

      // 规则 1 的一半：每一条 202 MUST 在册。
      if (!Object.hasOwn(registry, row.key)) {
        errors.push(
          `${docPath}: ${row.key} answers ${ACCEPTED_STATUS_KEY} with a job id, but the job-surface registry does not declare it — a route that mints a job id nobody registered is exactly the silence this closure exists to remove`,
        );
      }
    }

    for (const [key, surface] of Object.entries(registry)) {
      // 规则 1 的另一半：在册项 MUST 真的是一条 202 操作。
      if (!acceptingKeys.has(key)) {
        errors.push(
          `${docPath}: the job-surface registry declares ${key}, but it does not answer ${ACCEPTED_STATUS_KEY} — a registry entry with no accepting operation`,
        );
        continue;
      }
      if (!surface || typeof surface !== 'object' || Array.isArray(surface)) {
        errors.push(`${docPath}: the job-surface registry entry for ${key} is not an object`);
        continue;
      }

      // 规则 3：kind 与 blockedBy 恰有其一。
      const hasKind = Object.hasOwn(surface, 'kind');
      const hasBlocker = Object.hasOwn(surface, 'blockedBy');
      if (hasKind === hasBlocker) {
        errors.push(
          hasKind
            ? `${docPath}: ${key} declares both kind and blockedBy — a job either has a state source or a named reason it has none, never both, and a reader cannot tell which claim to trust`
            : `${docPath}: ${key} declares neither kind nor blockedBy — an entry that answers neither question is the silence this closure exists to remove`,
        );
        continue;
      }

      // 规则 4：blockedBy 落闭集；规则 6 的一半：已实现的路由 MUST NOT 声称无状态来源。
      if (hasBlocker) {
        if (typeof surface.blockedBy !== 'string' || !Object.hasOwn(JOB_BLOCKERS, surface.blockedBy)) {
          errors.push(
            `${docPath}: ${key} names blockedBy ${JSON.stringify(surface.blockedBy)}, which is not one of ${Object.keys(JOB_BLOCKERS).join(', ')} — an unnamed reason is indistinguishable from an unexamined one`,
          );
        } else if (declared.includes(key)) {
          errors.push(
            `${docPath}: ${key} is declared implemented but claims blockedBy ${JSON.stringify(surface.blockedBy)} — an implemented route that mints no job id contradicts its own declaration`,
          );
        }
      }

      // 规则 5 的一半：kind MUST 有状态来源；规则 6 的另一半：有 kind ⇒ MUST 已实现。
      if (hasKind) {
        if (typeof surface.kind !== 'string' || !Object.hasOwn(JOB_KINDS, surface.kind)) {
          errors.push(
            `${docPath}: ${key} names kind ${JSON.stringify(surface.kind)}, for which JOB_KINDS declares no journal and no reader — polling that job id would reach a state source nobody named`,
          );
        } else if (!declared.includes(key)) {
          errors.push(
            `${docPath}: ${key} claims kind ${JSON.stringify(surface.kind)} but is not declared implemented — a job that can be polled but never started is a state source with no producer`,
          );
        }
      }
    }

    // 规则 7：`JobAccepted.status` 的取值域，是「accepted ≠ 可轮询」这句承诺的对端。
    const jobAccepted = document.components?.schemas?.JobAccepted;
    if (jobAccepted) {
      sawJobAccepted = true;
      const statusEnum = jobAccepted.properties?.status?.enum;
      if (
        !Array.isArray(statusEnum) ||
        statusEnum.length !== 1 ||
        statusEnum[0] !== ACCEPTED_STATUS_ONLY
      ) {
        errors.push(
          `${docPath}: JobAccepted.status is ${JSON.stringify(
            statusEnum ?? null,
          )}, expected exactly ${JSON.stringify([ACCEPTED_STATUS_ONLY])} — any second value would let an accepted-but-unpollable job claim through its own body that it is progressing`,
        );
      }
    } else if (accepting.length > 0) {
      errors.push(
        `${docPath}: ${accepting.length} operations answer ${ACCEPTED_STATUS_KEY} but the document defines no JobAccepted schema — every ${JOB_ACCEPTED_SCHEMA} reference dangles`,
      );
    }

    // 规则 5 的另一半：`JobView.kind` 的枚举 MUST 与 JOB_KINDS 双向相等。
    const jobView = document.components?.schemas?.JobView;
    if (jobView) {
      sawJobView = true;
      const kindEnum = jobView.properties?.kind?.enum;
      if (!Array.isArray(kindEnum)) {
        errors.push(
          `${docPath}: JobView.kind declares no enum — the shared job id space has no discriminator, so a job id cannot say which table answers for it`,
        );
      } else {
        for (const kind of Object.keys(JOB_KINDS)) {
          if (!kindEnum.includes(kind)) {
            errors.push(
              `${docPath}: JOB_KINDS declares kind ${JSON.stringify(
                kind,
              )} (it has a journal and a reader), but JobView.kind does not list it — a journal no reader can name`,
            );
          }
        }
        for (const value of kindEnum) {
          if (!Object.hasOwn(JOB_KINDS, value)) {
            errors.push(
              `${docPath}: JobView.kind lists ${JSON.stringify(
                value,
              )}, but JOB_KINDS declares no journal and no reader for it — a contract branch that reaches nothing`,
            );
          }
        }
      }
    }
  }

  if (!anySurface) {
    errors.push(
      'no document declares a job surface — every rule above would be vacuously true, and the closure would read as "nothing to check"',
    );
  }
  if (!sawJobAccepted) {
    errors.push(
      `no OpenAPI document defines the JobAccepted schema — the shape every ${ACCEPTED_STATUS_KEY} body is supposed to share is gone`,
    );
  }
  if (!sawJobView) {
    errors.push(
      'no OpenAPI document defines the JobView schema — the discriminator that makes the shared job id space decidable is gone',
    );
  }

  return errors;
}

/// 校验一条 `{file, text}` 锚点：文件 MUST 被跟踪、MUST 可读、且那段文本 MUST 还在。
///
/// 三种失败 MUST 分开报：「文件没被跟踪」（锚点指向仓库外）、「文件读不出来」
/// （不知道 ≠ 没有）、「文本不在了」（声明失去对端）。折成一条会丢掉**到底哪一步断了**。
function anchorErrors(errors, label, anchor, tracked, readSource) {
  if (
    !anchor ||
    typeof anchor !== 'object' ||
    typeof anchor.file !== 'string' ||
    anchor.file.length === 0 ||
    typeof anchor.text !== 'string' ||
    anchor.text.length === 0
  ) {
    errors.push(
      `${label} has no usable source anchor — the claim would be a comment, and a comment never turns red`,
    );
    return;
  }
  if (!tracked.has(anchor.file)) {
    errors.push(
      `${label} anchors on ${anchor.file}, which the repository does not track — an anchor outside the tracked tree decides nothing`,
    );
    return;
  }
  const source = readSource(anchor.file);
  if (typeof source !== 'string') {
    errors.push(
      `${label} anchors on ${anchor.file}, but that file could not be read — "could not read" and "text is gone" are different answers`,
    );
    return;
  }
  if (!source.includes(anchor.text)) {
    errors.push(
      `${label} anchors on ${JSON.stringify(anchor.text)} in ${anchor.file}, and that text is no longer there — the declaration lost its other end`,
    );
  }
}

/// **仓库侧主判据**：每个 `kind` 的 journal／reader／writer、每个 `blockedBy` 的锚点、
/// 每个 `UpgradeHost` 实现者，MUST 都还在仓库里被跟踪着；登记表与它的执行侧消费者
/// MUST 是**同一份字节**。
///
/// 入参是一个**具名对象**（W146 从位置参数改成对象）：这一轮之后它要同时喂
/// 「状态来源」「拒绝理由」「安装器名册」与「契约侧登记表」四张表，位置参数会变成
/// 「第四个是什么来着」——而一条按错位参数判出来的绿，比没有判据更坏。
/// 六项**都是必需**，缺一项即具名失败：
/// - `kinds` / `blockers` / `hosts` 是待验的声明（默认值在调用方给，MUST NOT 在这里兜默认，
///   否则注入一份空表就变成「没有声明」而不是「声明被清空了」）；
/// - `surfaces` 是登记表（`tests/job-surface.json` 解析后的对象），用来判「能轮询的作业面 ⇔
///   生产安装器」这一对；
/// - `tracked` 是已跟踪路径集（`Set`）；
/// - `readSource` 是 `(path) => string | null`。
///
/// `readSource` 由调用方注入而不是在本模块里 `readFileSync`：这个模块只判定，
/// 不做 I/O，判定本身才能被合成输入驱动（真实仓库用例与闭世界各跑一次）。
export function jobSourceErrors({ kinds, blockers, hosts, surfaces, tracked, readSource } = {}) {
  const errors = [];

  if (!tracked || typeof tracked.has !== 'function' || typeof tracked[Symbol.iterator] !== 'function') {
    errors.push(
      'the source closure requires the tracked-path set and it was not provided — "not tracked" and "not given" are different answers, and folding them would silently pass',
    );
    return errors;
  }
  if (typeof readSource !== 'function') {
    errors.push(
      'the source closure requires a reader for tracked files and it was not provided — an anchor nobody reads is a comment',
    );
    return errors;
  }
  if (!kinds || typeof kinds !== 'object' || Array.isArray(kinds)) {
    errors.push('the source closure requires the kinds declaration and it was not provided');
    return errors;
  }
  if (!blockers || typeof blockers !== 'object' || Array.isArray(blockers)) {
    errors.push('the source closure requires the blockers declaration and it was not provided');
    return errors;
  }
  if (!hosts || typeof hosts !== 'object' || Array.isArray(hosts)) {
    errors.push(
      'the source closure requires the host registry and it was not provided — an absent roster and an empty one are different answers, and folding them would report "no installer was ever declared" as a pass',
    );
    return errors;
  }
  if (!surfaces || typeof surfaces !== 'object' || Array.isArray(surfaces)) {
    errors.push(
      'the source closure requires the job-surface registry and it was not provided — the pair rule (a pollable job needs a production installer) cannot be decided without it',
    );
    return errors;
  }

  if (Object.keys(kinds).length === 0) {
    errors.push(
      'JOB_KINDS declares no kind — the shared job id space would have no state source at all, which the 15 accepting operations quietly depend on',
    );
  }
  if (Object.keys(blockers).length === 0) {
    errors.push(
      'JOB_BLOCKERS declares no class — every blockedBy value would be rejected by the surface rules, and the reasons themselves would be unverifiable',
    );
  }
  if (Object.keys(hosts).length === 0) {
    errors.push(
      'JOB_HOSTS declares no host — "there is no production installer" would be an empty roster, which is exactly the silence this round is removing',
    );
  }

  // **写入侧的扫描域先读一遍**（W146）。
  //
  // 读不出来的文件 MUST 报成「读不出来」，MUST NOT 当成「里面没有写入点」：后者会让
  // 「扫描域被弄瞎」与「状态无处可写」共用同一条错误——两条相反的事实同一个信号。
  const writeSources = new Map();
  for (const path of tracked) {
    if (typeof path !== 'string') continue;
    if (!WRITE_SCAN_EXTENSIONS.some(extension => path.endsWith(extension))) continue;
    const text = readSource(path);
    if (typeof text !== 'string') {
      errors.push(
        `${path} is inside the write-side scan domain but could not be read — "could not be read" and "declares no writer" are different answers`,
      );
      continue;
    }
    writeSources.set(path, text);
  }
  if (writeSources.size === 0) {
    errors.push(
      `the write-side scan domain (${WRITE_SCAN_EXTENSIONS.join(', ')}) holds no readable tracked file — scanning nothing and finding no writer are different answers`,
    );
  }

  for (const [kind, source] of Object.entries(kinds)) {
    if (!source || typeof source !== 'object' || Array.isArray(source)) {
      errors.push(`JOB_KINDS[${kind}] is not an object, so its state source cannot be checked`);
      continue;
    }
    anchorErrors(errors, `JOB_KINDS[${kind}].journal`, source.journal, tracked, readSource);
    anchorErrors(errors, `JOB_KINDS[${kind}].reader`, source.reader, tracked, readSource);

    // 规则 8：**谁写这行**。锚点证明写入函数还在；`statement` 的计数证明**只有它**能写。
    const writer = source.writer;
    anchorErrors(errors, `JOB_KINDS[${kind}].writer`, writer, tracked, readSource);
    if (!writer || typeof writer !== 'object' || Array.isArray(writer)) {
      errors.push(`JOB_KINDS[${kind}] has no writer — a state source nobody writes is unreadable in practice`);
      continue;
    }
    if (typeof writer.statement !== 'string' || writer.statement.trim().length === 0) {
      errors.push(
        `JOB_KINDS[${kind}].writer has no statement — without the verbatim write it makes, "there is exactly one writer" cannot be judged, only believed`,
      );
      continue;
    }
    const writers = [];
    for (const [path, text] of writeSources) {
      const occurrences = text.split(writer.statement).length - 1;
      for (let index = 0; index < occurrences; index += 1) writers.push(path);
    }
    if (writers.length === 0) {
      errors.push(
        `JOB_KINDS[${kind}] names ${JSON.stringify(
          writer.statement,
        )} as its write, and that statement appears nowhere in the tracked scan domain — the journal of this kind can never receive a row, so every /jobs/{id} for it is a 404 by construction`,
      );
    } else if (writers.length > 1) {
      errors.push(
        `JOB_KINDS[${kind}] names ${JSON.stringify(writer.statement)} as its write, and it appears ${
          writers.length
        } times (${[...new Set(writers)].join(', ')}) — a second write path bypasses the journal it is supposed to go through, and "who wrote this row" stops having one answer`,
      );
    } else if (writers[0] !== writer.file) {
      errors.push(
        `JOB_KINDS[${kind}] declares its writer in ${writer.file}, but the only ${JSON.stringify(
          writer.statement,
        )} in the tracked tree is in ${writers[0]} — the declaration points at a file that does not write`,
      );
    }
  }

  // 规则 9：**安装器名册**。扫描到的 `impl UpgradeHost for X` MUST 与名册双向相等。
  //
  // 单向不够：只判「名册里的都有实现」会漏掉「仓库里多了个实现而名册没写」——
  // 那正是本轮要防的那件事（生产安装器落地而登记表不动）。
  const implementedHosts = new Map();
  for (const [path, text] of writeSources) {
    if (!path.endsWith('.rs')) continue;
    for (const match of text.matchAll(HOST_IMPL)) {
      const name = match[1];
      if (!implementedHosts.has(name)) implementedHosts.set(name, []);
      implementedHosts.get(name).push(path);
    }
  }
  const productionHosts = [];
  for (const [name, entry] of Object.entries(hosts)) {
    if (!entry || typeof entry !== 'object' || Array.isArray(entry)) {
      errors.push(`JOB_HOSTS[${name}] is not an object, so its claim cannot be checked`);
      continue;
    }
    // `production` MUST 是布尔：用「名字里没有 Rehearsal」去推断，会在第一个
    // 叫 `PlatformHost` 的演习替身出现时错得没有信号。
    if (typeof entry.production !== 'boolean') {
      errors.push(
        `JOB_HOSTS[${name}].production is ${JSON.stringify(
          entry.production ?? null,
        )}, expected a boolean — "unknown" and "not production" are different answers, and the pair rule below reads this value`,
      );
    } else if (entry.production) {
      productionHosts.push(name);
    }
    if (typeof entry.why !== 'string' || entry.why.trim().length === 0) {
      errors.push(`JOB_HOSTS[${name}] has no why — a roster entry with no stated role is indistinguishable from an unexamined one`);
    }
    if (typeof entry.file !== 'string' || entry.file.length === 0) {
      errors.push(`JOB_HOSTS[${name}] does not name its file — the roster entry would have nothing to point at`);
    } else if (!tracked.has(entry.file)) {
      errors.push(`JOB_HOSTS[${name}] points at ${entry.file}, which the repository does not track`);
    }
    if (!implementedHosts.has(name)) {
      errors.push(
        `JOB_HOSTS declares ${name}, but no tracked file contains "impl UpgradeHost for ${name}" — a roster entry with no implementation, so the roster would be describing a world that does not exist`,
      );
    }
  }
  for (const [name, paths] of implementedHosts) {
    if (!Object.hasOwn(hosts, name)) {
      errors.push(
        `${name} implements UpgradeHost in ${[...new Set(paths)].join(', ')}, but JOB_HOSTS does not declare it — an installer nobody registered is an installer whose production status nobody decided`,
      );
    }
  }

  // 规则 10：**成对**——「能被轮询的作业面」⇔「生产安装器」。
  //
  // 两条今天都空转（登记表 0 条 `kind`、名册 0 个生产实现），这是**如实**的：
  // 它们不是给今天看的，是给下一跳的。任何一侧单独动，另一侧没跟上，立刻红：
  // - 加一条 `kind` 而没有生产安装器 ⇒ 作业永远没有行，`/jobs/{id}` 的 200 是假承诺；
  // - 加了生产安装器而没有 `kind` ⇒ 能力落地了，契约还说每条作业都被挡着。
  const kindSurfaces = [];
  for (const [docPath, registry] of Object.entries(surfaces)) {
    if (!registry || typeof registry !== 'object' || Array.isArray(registry)) continue;
    for (const [key, surface] of Object.entries(registry)) {
      if (surface && typeof surface === 'object' && !Array.isArray(surface) && Object.hasOwn(surface, 'kind')) {
        kindSurfaces.push(`${docPath}: ${key}`);
      }
    }
  }
  if (kindSurfaces.length > 0 && productionHosts.length === 0) {
    errors.push(
      `${kindSurfaces.length} job surface(s) claim a kind (${kindSurfaces.join(', ')}) but JOB_HOSTS declares no production host — nothing would ever advance those transactions, so their journal rows are never written and "you can poll this job" would be false`,
    );
  }
  if (productionHosts.length > 0 && kindSurfaces.length === 0) {
    errors.push(
      `JOB_HOSTS declares production host(s) ${productionHosts.join(
        ', ',
      )} but no job surface claims a kind — an installer landed while the registry still says every job is blocked, so either the registry is stale or that host is built and never used`,
    );
  }

  for (const [blocker, entry] of Object.entries(blockers)) {
    if (!entry || typeof entry !== 'object' || Array.isArray(entry)) {
      errors.push(`JOB_BLOCKERS[${blocker}] is not an object`);
      continue;
    }
    // `why` 不是装饰：一个说不出理由的类别，读起来和「没看过」一样。
    if (typeof entry.why !== 'string' || entry.why.trim().length === 0) {
      errors.push(`JOB_BLOCKERS[${blocker}] has no why — an unnamed reason is indistinguishable from an unexamined one`);
    }
    anchorErrors(errors, `JOB_BLOCKERS[${blocker}].anchor`, entry.anchor, tracked, readSource);
  }

  // **单一枢纽**：执行侧 MUST 读到同一份字节（W145）。
  //
  // 这一条比任何一处锚点断裂都安静：两侧各自全绿、判的却是两份数据。它也不能靠
  // 「Rust 里有个 include_str!」来满足——那只能证明它读**某个**文件；路径必须逐字对上。
  if (!tracked.has(JOB_REGISTRY.path)) {
    errors.push(
      `the job-surface registry ${JOB_REGISTRY.path} is not tracked — a clean checkout would not have it, so the execution-side probe and this judge would be reading different trees`,
    );
  }
  const consumer = JOB_REGISTRY.consumer;
  if (!tracked.has(consumer.file)) {
    errors.push(
      `the execution-side consumer ${consumer.file} is not tracked — the registry has no reader in the tracked tree, so nothing proves the classification against the router`,
    );
  } else {
    const consumerSource = readSource(consumer.file);
    if (typeof consumerSource !== 'string') {
      errors.push(
        `${consumer.file} could not be read — "could not read" and "does not embed the registry" are different answers`,
      );
    } else if (!consumerSource.includes(consumer.text)) {
      errors.push(
        `${consumer.file} does not embed ${JSON.stringify(
          consumer.text,
        )} — the execution-side probe is reading some other file, and both sides could stay green while judging different data`,
      );
    }
  }

  return errors;
}
