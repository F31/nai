/// **成因兑付**：`ci-only` 与 `standalone` 的每一条成因 MUST **付代价**——它 MUST 给出一条
/// **可从仓库自证**的凭据：一串在仓库里**逐字存在**的文本。没有凭据的成因是一句**无从查证的
/// 话**；没有实例的成因格是一条**对空集恒真**的规则。
///
/// 为什么需要它：`CI_ONLY_REASONS`（W95）与 `STANDALONE_REASONS`（W96）把「为什么它不在门控里」
/// 收进了一个闭集，但判据对闭集**之内**的每一格只做一次 `includes()`——它答「这个成因**在不在**
/// 表里」，**不答**「这个成因**凭什么成立**」。于是：
/// - **成因本身**没有仓库内的凭据：`superseded-by-workspace-run` 断言「门控跑的是全 workspace 测试」，
///   这句话若哪天不再成立（那条 `--workspace` 调用被换成显式包列表），成因格照旧成立、判据照旧全绿；
/// - **成因格**没有「至少有一个实例」的断言：某一格若被**合法清空**，格子仍在常量里，规则对空集恒真。
///
/// 直接触发是 ADR 0073 遗留 1 预言的失效：该 ADR 逐字写着「`schema --check` 哪天在本机变得可跑，
/// 判据不会有信号」。2026-10-14 它**真的**在本机跑通了，而 `reason` 仍写着 `host-subprocess`，
/// 全部判据零信号 ⇒ **一条会随仓库自身进展而过期的声明，没有任何判据盯着它**（遗忘的形状）。
///
/// ⚠️ **处置 MUST NOT 是「宿主现在能跑了」**：宿主瞬态 MUST NOT 改变被测系统的结论（铁律二），
/// 换一台机器会对同一个仓库得出不同分类。⇒ 正确的问题是「它**能不能**从仓库自证」——
/// 不能的成因（`host-subprocess`：宿主能不能派生子进程）**不可能有凭据**，它是这条规则里唯一一个
/// **永久无法兑付的空格**，必须逐出。逐出的理由**不引用任何发生过的瞬态**。
///
/// ⚠️ **判据验的是「凭据存在」，不是「凭据与成因语义一致」**（如实披露）：前者可判，
/// 后者是人的判断且成因是三合一的（仓库事实 + 跨仓库契约 + 触发方式）。
/// ⇒ `why` 字段把「这条凭据回答了什么」写到明面上，**判据读 `marker`，人读 `why`**。
///
/// **两张表不是同一件事的第二处定义**：两处的成因闭集**本就不同**（命令层回答「这条命令为什么不在
/// 本机门控闭包里」，step 层回答「这一步为什么只有 CI 跑」），判定单元也不同（签名 vs 身份）。
/// 但**核验逻辑是同一套** ⇒ 住在这里，两处各自调用。

/// 凭据 MUST NOT 落在这个前缀下：那是**被审注册表自己所在的目录**。
/// 表自己写的凭据对表恒真——**自我满足**的典型形态。
export const PAYMENT_EXCLUDED_PREFIX = 'scripts/contracts/';

/// 成因兑付的核验。**入参显式注入**：缺任一项即报错（一条会因入参缺失而恒真的断言比没有断言更坏）。
///
/// - `label`：报错前缀（被审宿主的路径）；
/// - `where`：报错里指代被断言对象的词（`command` / `step`）；
/// - `closedSet`：成因闭集；
/// - `payments`：兑付表 `{cause: {evidence: [{file, marker, why}, …]}}`；
/// - `reasons`：在册条目的成因（**含重复**，用于「格非空洞」）；
/// - `sources`：`Map<path, text>`——凭据文件的内容，**显式注入**（少了它，「凭据逐字存在」会退化）；
/// - `tracked`：`Set<string>`——**已跟踪**路径（`target/` 下的产物、工作树里的临时文件不算仓库事实）。
export function reasonPaymentErrors({label, where, closedSet, payments, reasons, sources, tracked}) {
  if (!Array.isArray(closedSet) || closedSet.length === 0) {
    return [`${label}: the cause table was not provided or is empty, so no cause can be judged`];
  }
  if (payments === null || typeof payments !== 'object' || Array.isArray(payments)) {
    return [`${label}: the cause payment table was not provided, so no cause can be paid for`];
  }
  if (!(sources instanceof Map)) {
    return [
      `${label}: the cause evidence sources were not provided as a map, so a receipt cannot be told from a claim`,
    ];
  }
  if (!(tracked instanceof Set)) {
    return [
      `${label}: the tracked-path set was not provided, so repository content cannot be told from a stray file`,
    ];
  }

  const errors = [];

  // ---------- 双向：成因闭集 ↔ 兑付表。两个方向各自具名（欠付 / 空规格是两件事） ----------
  const paid = new Set(Object.keys(payments));
  for (const cause of closedSet) {
    if (!paid.has(cause)) {
      errors.push(
        `${label}: the cause "${cause}" is in the closed set but no payment says what it stands on — ` +
          'a cause nobody pays for is a label, not a reason',
      );
    }
  }
  for (const cause of paid) {
    if (!closedSet.includes(cause)) {
      errors.push(
        `${label}: ${JSON.stringify(cause)} declares a payment but is not in the cause table — ` +
          'a payment with no cause is dead weight',
      );
    }
  }

  // ---------- 每条规格：凭据 ----------
  for (const [cause, payment] of Object.entries(payments)) {
    const evidence = payment?.evidence;
    if (!Array.isArray(evidence) || evidence.length === 0) {
      errors.push(
        `${label}: the cause "${cause}" pays for itself with no evidence at all — ` +
          '"this is why it is not in the gate" has to be a fact, not a tone',
      );
      continue;
    }
    for (const item of evidence) {
      if (
        typeof item?.file !== 'string' ||
        item.file === '' ||
        typeof item.marker !== 'string' ||
        item.marker === ''
      ) {
        errors.push(
          `${label}: the cause "${cause}" carries a receipt that is not a (file, marker) pair: ` +
            JSON.stringify(item),
        );
        continue;
      }
      // `why` 不阻断后面的检查——它答的是「这条凭据回答了哪个问题」，与凭据本身是否在仓库里无关。
      if (typeof item.why !== 'string' || item.why.trim() === '') {
        errors.push(
          `${label}: the receipt ${JSON.stringify(item.file)} for cause "${cause}" does not say what it ` +
            'answers — a receipt which answers no question is a decoration',
        );
      }
      if (item.file.startsWith(PAYMENT_EXCLUDED_PREFIX)) {
        errors.push(
          `${label}: the cause "${cause}" is paid for by ${item.file}, which the registry itself writes — ` +
            'a receipt written by the table is true by construction',
        );
        continue;
      }
      if (!tracked.has(item.file)) {
        errors.push(
          `${label}: the cause "${cause}" is paid for by ${item.file}, which the repository does not track — ` +
            'a readable file in the working tree is not repository content',
        );
        continue;
      }
      if (!sources.has(item.file)) {
        // 缺输入 MUST 具名，MUST NOT 静默跳过——那会让判据在被削弱时保持沉默。
        errors.push(`${label}: the cause "${cause}" cites ${item.file}, which was not provided for judgement`);
        continue;
      }
      if (!sources.get(item.file).includes(item.marker)) {
        errors.push(
          `${label}: the cause "${cause}" cites ${item.file} for ${JSON.stringify(item.marker)}, ` +
            'which is not in that file — the receipt is a claim, not a fact',
        );
      }
    }
  }

  // ---------- 格非空洞：每个成因取值 MUST ≥1 在册条目。
  // 一条**没有实例**的规则对空集恒真——它会在读起来「有这一格」的同时什么也不管。
  const used = new Set(reasons);
  for (const cause of closedSet) {
    if (!used.has(cause)) {
      errors.push(
        `${label}: no ${where} in the registry carries the cause ${JSON.stringify(cause)} — ` +
          'an empty cause cell makes its rules vacuously true',
      );
    }
  }

  return errors;
}
