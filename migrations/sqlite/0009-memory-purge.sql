-- W111: 记忆载荷的清除时刻（§42.4 清理侧）。
--
-- 为什么加列而不是直接删行：§24.4「删除先发布 tombstone，使所有待播内容失效」——
-- 行本身是「这条记录被遗忘过」的**可验证凭据**，被清掉的是 `content`。
--
-- 为什么可空：未清除 MUST 是 NULL，**MUST NOT** 是 0。0 会被 `purged_at_ms.is_some()`
-- 之外的任何按值比较读成「1970 年就已清」，也会让「已清」与「未清」在老行上无法区分。
-- 这与 0008 的 `revoked_at_ms` 是同一个理由，故沿用它的形状。
ALTER TABLE memory_records ADD COLUMN purged_at_ms INTEGER;