// 升级成本显示的可测纯逻辑（§1330 O05；V3.7 §14/§15「成本逐项列示」）。
//
// 与 `console.ts` 同形：**不碰 DOM**。要能被机器检查的性质——「未知不等于零」「部分和不当作总额」——
// 必须脱离渲染才能断言；组件只把 `costRows()` / `totalPresentation()` 的结果贴回界面。
// 显式 `.ts` 扩展名：Node 以 `--experimental-strip-types` 直接加载本模块（ESM 不做扩展名补全）。
//
// 载荷形状镜像 `nai-upgrade-core::cost` 的语义：每个分项是「已知的金额」或「未知及其原因」，
// 二者择一，不存在第三种。逐项列示的四类是云资源、下载、存储与预估运行增量。

/** 分项标识与界面标签。标识与 `CostEstimate::components()` 的报告顺序一致。 */
export const COST_COMPONENTS = [
  { id: 'cloud', label: '云资源' },
  { id: 'download', label: '下载' },
  { id: 'storage', label: '存储' },
  { id: 'runtime_delta', label: '运行增量' },
] as const;

export type CostComponentId = (typeof COST_COMPONENTS)[number]['id'];

/** 一个分项：已知金额（最小货币单位），或未知及其**原因**。 */
export type CostLine = { known: number } | { unknown: string };

export type CostEstimate = {
  currency: string;
  components: Record<CostComponentId, CostLine>;
};

/** 一行呈现：`label：text` 由界面拼装，`text` 本身已含语义。 */
export type CostRow = { id: CostComponentId; label: string; text: string; known: boolean };

export type TotalPresentation = {
  text: string;
  complete: boolean;
  floor: number;
  unknown: number;
};

/** 金额以最小货币单位表示——这一点必须说出来，否则与「元」混淆。 */
export const COST_UNITS_NOTE = '金额为最小货币单位';

/** 含未知分项时总额的措辞：说「下界」，并说明**不是**总额。 */
export const COST_PARTIAL_NOTE = '非总额';

/**
 * 成本来源的边界，必须与金额一起呈现。
 *
 * 控制面的 `/upgrades/plan` 只收**包清单**、不验签（清单里没有包体，验签在 execute 侧）。
 * 于是这里算出来的是**厂商声明值**，不是「已核实的收费」。把这句话省掉，
 * 界面就把「别人说多少钱」显示成了「要花多少钱」——同一类不伪报，只是换了个位置。
 */
export const COST_DECLARED_BASIS_NOTE = '金额来自包清单声明，未验签；执行前由升级事务校验签名';

const LINE_KEYS = ['known', 'unknown'] as const;

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function parseLine(id: string, value: unknown): CostLine {
  if (!isPlainObject(value)) throw new Error(`成本载荷结构无效：分项 ${id} 不是对象`);
  const keys = Object.keys(value);
  const extra = keys.filter((key) => !LINE_KEYS.includes(key as (typeof LINE_KEYS)[number]));
  if (extra.length > 0) throw new Error(`成本载荷结构无效：分项 ${id} 含未知字段 ${extra.join(',')}`);
  if (keys.length !== 1) throw new Error(`成本载荷结构无效：分项 ${id} 必须恰好给出 known 或 unknown 之一`);
  if ('unknown' in value) {
    const reason = value.unknown;
    if (typeof reason !== 'string' || reason.trim().length === 0) {
      throw new Error(`成本载荷结构无效：分项 ${id} 声明为未知却未说明原因`);
    }
    return { unknown: reason };
  }
  const amount = value.known;
  if (typeof amount !== 'number' || !Number.isSafeInteger(amount) || amount < 0) {
    throw new Error(`成本载荷结构无效：分项 ${id} 的金额必须是非负安全整数（最小货币单位）`);
  }
  return { known: amount };
}

/**
 * 严格解析成本载荷：缺字段、未知字段、非法金额或空原因一律**具名拒绝**。
 * 不做静默补 0，也不把「拿不准」猜成「免费」。
 */
export function parseCostEstimate(value: unknown): CostEstimate {
  if (!isPlainObject(value)) throw new Error('成本载荷结构无效：不是对象');
  const extraTop = Object.keys(value).filter((key) => key !== 'currency' && key !== 'components');
  if (extraTop.length > 0) throw new Error(`成本载荷结构无效：含未知字段 ${extraTop.join(',')}`);
  const { currency, components } = value;
  if (typeof currency !== 'string' || !/^[A-Z]{3}$/.test(currency)) {
    throw new Error('成本载荷结构无效：currency 必须是三字母大写货币代码');
  }
  if (!isPlainObject(components)) throw new Error('成本载荷结构无效：components 不是对象');
  const ids = COST_COMPONENTS.map((component) => component.id);
  for (const key of Object.keys(components)) {
    if (!ids.includes(key as CostComponentId)) {
      throw new Error(`成本载荷结构无效：未知的分项 ${key}`);
    }
  }
  const parsed = {} as Record<CostComponentId, CostLine>;
  for (const id of ids) {
    if (!(id in components)) throw new Error(`成本载荷结构无效：缺少分项 ${id}`);
    parsed[id] = parseLine(id, components[id]);
  }
  return { currency, components: parsed };
}

/**
 * 逐项呈现。未知分项**呈现为原因**，绝不呈现为 0——
 * 0 是一个「这块免费」的断言，我们给不出；「未知」是可支持的真相。
 */
export function costRows(estimate: CostEstimate): CostRow[] {
  return COST_COMPONENTS.map(({ id, label }) => {
    const line = estimate.components[id];
    if ('unknown' in line) {
      return { id, label, known: false, text: `${label}：未知（${line.unknown}）` };
    }
    return { id, label, known: true, text: `${label}：${line.known} ${estimate.currency}` };
  });
}

/**
 * 总额呈现。含未知分项时给**下界 + 未知计数**并说明它**不是**总额；
 * 把部分和当作总额，就是分项层面已经避免的同一个「不伪报」违规，只是上升了一层。
 */
export function totalPresentation(estimate: CostEstimate): TotalPresentation {
  let floor = 0;
  let unknown = 0;
  for (const { id } of COST_COMPONENTS) {
    const line = estimate.components[id];
    if ('unknown' in line) unknown += 1;
    else floor += line.known;
  }
  if (unknown === 0) {
    return { text: `合计 ${floor} ${estimate.currency}`, complete: true, floor, unknown: 0 };
  }
  return {
    text: `下界 ${floor} ${estimate.currency}（${unknown} 项未知，${COST_PARTIAL_NOTE}）`,
    complete: false,
    floor,
    unknown,
  };
}
