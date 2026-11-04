// Legacy backend smoke protocol parsing only; no UI/runtime entry point.
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

