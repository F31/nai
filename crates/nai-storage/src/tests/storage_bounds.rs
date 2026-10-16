//! R08 「存储维护有界」：W125 能把名册**当幅度钉住**（常量是不是它自称的那个数），
//! 但名册里还有 11 项**零覆盖**——没有一条判据知道它们的**闸门还在不在、边界在哪一侧**。
//! 这一组补的就是那一半。
//!
//! 三条纪律：
//! 1. **夹具从常量派生**，所以「抬高常量」不会让边界判据变红——那是幅度钉的职责
//!    （`tests::the_declared_storage_write_bounds_are_the_magnitudes_the_item_claims`）；
//!    这里管的是「恰在界上必须过、越界必须**具名**拒绝」。
//! 2. **一个 `if` 里的多个条件各自举证**。凡一条拒绝名覆盖两个条件的地方，判据 MUST
//!    逐条件给夹具，否则「绊到其中一个」会替另一个背书——那正是 W125 把
//!    `publication` 的合并拒绝拆名的理由，⑥ 这一轮照做。
//! 3. **「恰能走到它那一层」**：每条断言都要先用一条更靠近的闸门证明自己没被更早的
//!    检查短路（例如 ⑪ 的十六进制长度闸门前面还有 ⑩ 的文本长度闸门）。否则注入那条
//!    断言时得到的「全绿」只证明它没被触发，不证明它会拦（W127）。

use super::*;
use prost::Message as _;
use rusqlite::params;

/// 一份内容随 `body` 变的文档：`document_revision` 把内容哈希折进去，所以重新导入
/// **铸造的是新版本**而不是改写同一个版本——而「版本」正是撤销批次要数的那样东西。
fn revised_document(document: &str, body: &str) -> crate::ImportedDocument {
    let mut doc = synthetic_document(
        document,
        vec![synthetic_chunk(document, 0, body.to_string())],
    );
    doc.content = body.to_string();
    doc
}

fn count(storage: &Store, sql: &str) -> i64 {
    storage.conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

/// 撤销批次数的是**表里的**不重复 (document, revision)，不是夹具自己的记账。
fn distinct_revisions(storage: &Store) -> i64 {
    count(
        storage,
        "SELECT COUNT(*) FROM (SELECT DISTINCT document_id,revision FROM knowledge_snapshot_documents \
         WHERE workspace_id='test-workspace')",
    )
}

fn grants_budget(storage: &mut Store) {
    storage
        .set_permission(
            &op(),
            "test-workspace",
            "operator",
            "budget.manage",
            "*",
            true,
        )
        .unwrap();
}

/// 一根指挥棒：每个上限都配一对夹具（恰在界上 / 越一字节）。
fn refusal(store: crate::StorageError, name: &str, code: nai_contracts::v1::ErrorCode) {
    assert_eq!(store.reason, name, "越界必须**具名**拒绝，不是静默截断");
    assert_eq!(store.code, code, "{name} 的错误码是它自己的那一类");
}

/// 发一份填充到 `pad` 字节的事实。写成函数而不是闭包：闭包会把 `storage` 借成可变，
/// 后面的只读就再也拿不到了。
///
/// ⚠️ 这份夹具是 `publish_fact` 那三处改写（`revision` / `publisher_ref` /
/// `permission_revision`）的**不动点**——它把这三个字段预先设成改写之后的值，于是
/// 「本地量到的编码」== 「落库的编码」。这个等式在用例里被**断言**（落库字节 == 本地测量），
/// 所以镜像一旦跟生产代码分叉，用例会红，而不是悄悄测错东西。
fn fill_fact(storage: &Store, pad: usize) -> nai_contracts::v1::AuthoritativeFact {
    // `workspaces.permission_revision` 是 INTEGER，`publish_fact` 读成 `i64` 再 `.to_string()`
    // 写回事实——所以夹具照同一路走，否则量到的不是同一个形状。
    let permission_revision: i64 = storage
        .conn
        .query_row(
            "SELECT permission_revision FROM workspaces WHERE workspace_id='test-workspace'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut candidate = fact();
    candidate.product_id = format!("sku-fill-{pad}");
    candidate.revision = "1".into();
    candidate.publisher_ref = "operator".into();
    candidate.permission_revision = permission_revision.to_string();
    candidate.value = Some(authoritative_fact::Value::Text("x".repeat(pad)));
    candidate
}

fn publish_fact_filled(
    storage: &mut Store,
    pad: usize,
) -> crate::Result<nai_contracts::v1::AuthoritativeFact> {
    let candidate = fill_fact(storage, pad);
    storage.publish_fact(&op(), "operator", &candidate, None)
}

/// R08 ④ + ⑤：限定**单份被写入的文档**在落到 SQLite 之前的两条编码上限。
#[test]
fn the_configuration_and_fact_encoding_bounds_are_refused_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);

    // ---- ④ configuration too large ----
    let config_cap = crate::store::MAX_CONFIGURATION_BYTES;
    // `runtime.plugin_catalog` 是 schema 里唯一的自由字符串，于是 JSON 长度可以被推到
    // 界上而**不违反 schema**（先过 schema、再过大小闸门，顺序就是判据要走的顺序）。
    let mut runtime: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/contracts/runtime.json"
    ))
    .unwrap();
    runtime["runtime"]["plugin_catalog"] = serde_json::Value::String(String::new());
    let base = runtime.to_string().len();
    runtime["runtime"]["plugin_catalog"] = serde_json::Value::String("p".repeat(config_cap - base));
    assert_eq!(
        runtime.to_string().len(),
        config_cap,
        "夹具必须**恰在界上**，否则「越界被拒」可能只是「界被画错了」"
    );
    assert_eq!(
        storage
            .save_config(
                &op(),
                ("test-workspace", "operator"),
                ConfigKind::Runtime,
                "main",
                &runtime,
                0
            )
            .unwrap(),
        1,
        "恰在界上的配置被接受"
    );
    runtime["runtime"]["plugin_catalog"] =
        serde_json::Value::String("p".repeat(config_cap - base + 1));
    let error = storage
        .save_config(
            &op(),
            ("test-workspace", "operator"),
            ConfigKind::Runtime,
            "main",
            &runtime,
            1,
        )
        .unwrap_err();
    refusal(
        error,
        "configuration too large",
        nai_contracts::v1::ErrorCode::ResourceExhausted,
    );

    // ---- ⑤ fact too large ----
    // 边界精确可构造（见 `fill_fact`：夹具是改写的**不动点**），所以这里钉的是真的
    // 「恰在界上」而不是「边界在附近」。
    let fact_cap = crate::store::MAX_FACT_BYTES;
    let mut pad = fact_cap;
    while fill_fact(&storage, pad).encode_to_vec().len() > fact_cap {
        pad -= 1;
    }
    let at_cap = fill_fact(&storage, pad).encode_to_vec().len();
    let over_cap = fill_fact(&storage, pad + 1).encode_to_vec().len();
    assert_eq!(at_cap, fact_cap, "界上那份 MUST 真的恰在界上");
    assert!(over_cap > fact_cap, "越界那份 MUST 真的越界");

    publish_fact_filled(&mut storage, pad).unwrap();
    // 镜像没跑偏：生产代码落库的字节数 == 夹具本地量到的字节数。
    let stored: i64 = storage
        .conn
        .query_row(
            "SELECT length(fact_proto) FROM facts WHERE workspace_id='test-workspace' \
             AND product_id=?1 AND field_key='price'",
            params![format!("sku-fill-{pad}")],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        stored as usize, at_cap,
        "落库字节 MUST 与夹具测量一致，否则这条判据测的不是同一个形状"
    );
    refusal(
        publish_fact_filled(&mut storage, pad + 1).unwrap_err(),
        "fact too large",
        nai_contracts::v1::ErrorCode::ResourceExhausted,
    );
    // 拒绝发生在任何写入之前：越界那份没有留下任何行。
    assert_eq!(
        count(
            &storage,
            &format!(
                "SELECT COUNT(*) FROM facts WHERE workspace_id='test-workspace' \
                 AND product_id='sku-fill-{}'",
                pad + 1
            )
        ),
        0,
        "越界的事实 MUST NOT 留下半条记录"
    );
}

/// R08 ⑥ + ⑧：候选路径上有三条互不相干的容量上限。本轮之前其中两条共用一个拒绝名，
/// 于是「绊到依赖条数」的夹具会替「候选载荷过大」背书。
#[test]
fn the_candidate_bounds_are_separately_attributable() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();

    // ⑥ 依赖条数（这条闸门在事实解析之前，所以不需要任何事实在场）
    let crowded: Vec<(&str, &str, &str)> =
        vec![("sku", "price", "1"); crate::store::MAX_CANDIDATE_DEPENDENCIES + 1];
    let error = storage
        .record_candidate(&op(), "test-workspace", "operator", &candidate(), &crowded)
        .unwrap_err();
    refusal(
        error,
        "candidate dependency limit",
        nai_contracts::v1::ErrorCode::ResourceExhausted,
    );

    // ⑥ 候选编码：同一个 `if` 的另一半，MUST 有自己的名字与自己的夹具。
    let mut heavy = candidate();
    heavy.display_text = "x".repeat(crate::store::MAX_CANDIDATE_BYTES);
    assert!(heavy.encode_to_vec().len() > crate::store::MAX_CANDIDATE_BYTES);
    let error = storage
        .record_candidate(&op(), "test-workspace", "operator", &heavy, &[])
        .unwrap_err();
    refusal(
        error,
        "candidate payload limit",
        nai_contracts::v1::ErrorCode::ResourceExhausted,
    );

    // ⑧ 证据引用条数：不带任何依赖，所以 ⑥ 不可能是成因。
    let mut cited = candidate();
    cited.evidence_refs = (0..=crate::knowledge::MAX_EVIDENCE_REFS)
        .map(|index| format!("chunk:evidence-{index}"))
        .collect();
    let error = storage
        .record_candidate(&op(), "test-workspace", "operator", &cited, &[])
        .unwrap_err();
    refusal(
        error,
        "candidate evidence bound",
        nai_contracts::v1::ErrorCode::ResourceExhausted,
    );

    // 正对照：两条闸门**都恰好**踩在界上时，候选必须被接纳——否则上面三条拒绝可能
    // 只是「夹具本身是坏的」。
    let mut owned = Vec::new();
    for index in 0..crate::store::MAX_CANDIDATE_DEPENDENCIES {
        let mut published = fact();
        published.product_id = format!("sku-{index}");
        published.value = Some(authoritative_fact::Value::Integer(index as i64));
        storage
            .publish_fact(&op(), "operator", &published, None)
            .unwrap();
        owned.push((format!("sku-{index}"), "price".to_string(), "1".to_string()));
    }
    let borrowed: Vec<(&str, &str, &str)> = owned
        .iter()
        .map(|(product, field, revision)| (product.as_str(), field.as_str(), revision.as_str()))
        .collect();
    let mut at_cap = candidate();
    at_cap.evidence_refs = owned
        .iter()
        .map(|(product, _, _)| format!("fact:{product}:price:1"))
        .collect();
    assert_eq!(borrowed.len(), crate::store::MAX_CANDIDATE_DEPENDENCIES);
    assert_eq!(
        at_cap.evidence_refs.len(),
        crate::knowledge::MAX_EVIDENCE_REFS
    );
    storage
        .record_candidate(&op(), "test-workspace", "operator", &at_cap, &borrowed)
        .unwrap();
}

/// R08 ⑦：待审批容量数的是「**还没被批准的**候选」。第 200 条自己必须过得去，
/// 第 201 条才被拒——否则这条闸门就是「上限少一」。
#[test]
fn the_pending_candidate_capacity_is_a_count_of_unapproved_work() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "session-test")
        .unwrap();
    let cap = crate::store::MAX_PENDING_CANDIDATES;
    let live = "SELECT COUNT(*) FROM utterances WHERE workspace_id='test-workspace' \
                AND session_id='session-test'";

    for index in 0..cap {
        let mut cue = candidate();
        cue.utterance_id = format!("utterance-{index}");
        cue.display_text = format!("待审批文本 {index}");
        cue.speech_text = cue.display_text.clone();
        storage
            .record_candidate(&op(), "test-workspace", "operator", &cue, &[])
            .unwrap();
    }
    assert_eq!(count(&storage, live), cap, "恰在界上的那条候选 MUST 被接纳");

    let mut over = candidate();
    over.utterance_id = "utterance-over".into();
    over.display_text = "越界文本".into();
    over.speech_text = "越界文本".into();
    let error = storage
        .record_candidate(&op(), "test-workspace", "operator", &over, &[])
        .unwrap_err();
    refusal(
        error,
        "session pending approval capacity",
        nai_contracts::v1::ErrorCode::ResourceExhausted,
    );
    assert_eq!(count(&storage, live), cap, "被拒的候选 MUST NOT 留下行");
}

/// R08 ②：快照选中文档数的闸门。空选集与越界共用一条拒绝名（两个分支），所以两个
/// 分支各自举证；而「越界」那一侧故意混进一个**不存在**的文档，好让拒绝**只能**来自
/// 条数——不存在走的是另一条具名拒绝。
#[test]
fn the_snapshot_document_count_bound_is_attributable_to_the_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let cap = crate::publication::MAX_SNAPSHOT_DOCUMENTS;

    let error = stage(&mut storage, "snap-none", &[]).unwrap_err();
    refusal(
        error,
        "invalid snapshot documents",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );

    // 探针：先确认「不存在的文档」**真的是另一条拒绝**，否则下面那条断言拦不住
    // 「其实是被当成缺文档拒的」。
    let absence = stage(&mut storage, "snap-absent", &["doc-absent"]).unwrap_err();
    refusal(
        absence,
        "snapshot document absent or duplicate",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );

    let documents: Vec<String> = (0..cap).map(|index| format!("doc-{index}")).collect();
    for document in &documents {
        storage
            .import_document(
                &op(),
                "operator",
                &revised_document(document, &format!("{document} 的正文")),
            )
            .unwrap();
    }
    let at_cap: Vec<&str> = documents.iter().map(|value| value.as_str()).collect();
    stage(&mut storage, "snap-at-cap", &at_cap).unwrap();

    let mut over_cap = at_cap.clone();
    over_cap.push("doc-absent");
    let error = stage(&mut storage, "snap-over-cap", &over_cap).unwrap_err();
    refusal(
        error,
        "invalid snapshot documents",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );
}

/// R08 ③：托管文件数的闸门在**导出快照时**按**不重复路径**数。路径是内容寻址的，
/// 所以 513 份不同内容才是 513 条路径。
#[test]
fn the_managed_artifact_count_bound_counts_distinct_paths() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let cap = crate::snapshot::MAX_SNAPSHOT_ARTIFACTS;
    let paths = "SELECT COUNT(DISTINCT path) FROM managed_artifacts";

    for index in 0..cap {
        storage
            .publish_artifact(
                &op(),
                "test-workspace",
                "operator",
                &format!("artifact-{index}"),
                "assets",
                format!("payload-{index}").as_bytes(),
            )
            .unwrap();
    }
    assert_eq!(
        count(&storage, paths),
        cap as i64,
        "界上的每一份都占一条路径"
    );
    storage.backup(&op()).unwrap();

    storage
        .publish_artifact(
            &op(),
            "test-workspace",
            "operator",
            "artifact-over",
            "assets",
            b"payload-over",
        )
        .unwrap();
    assert_eq!(count(&storage, paths), cap as i64 + 1);
    let error = storage.backup(&op()).unwrap_err();
    refusal(
        error,
        "snapshot artifact count limit",
        nai_contracts::v1::ErrorCode::ResourceExhausted,
    );
}

/// R08 ⑨：写入侧的编码上限。**本轮实测发现它从被认可的路径走不到**——
/// `nai-budget-core` 的校验把每个字符串字段限在 256 字符、`units`/`concurrent` 限在
/// 32 个键 × 64 字符，于是最大的合法条目编码出来只有几 KB。设计不上手改（那是另一件事），
/// 但「它今天走不到」必须既被**说清楚**、又被一条**会过期**的断言钉住：
/// ① 闸门本身两侧都在——直接问 `budget::encode`（接缝级，MUST 如实标明不是路径级）；
/// ② 被认可的路径能装下的最大条目离上限多远——一旦哪天有字段能长过 256 字符，这条会红。
#[test]
fn the_ledger_write_bound_is_defensive_and_its_gate_has_two_sides() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    grants_budget(&mut storage);

    // ---- ① 接缝上的两侧 ----
    let cap = crate::budget::MAX_LEDGER_ENTRY_BYTES;
    let shape = |pad: usize| {
        let mut probe = budget_entry("a", "request-pad", 1);
        probe.request_id = "r".repeat(pad);
        probe.encode_to_vec().len()
    };
    let mut pad = cap;
    while shape(pad) > cap {
        pad -= 1;
    }
    assert!(shape(pad) <= cap, "界上 MUST 编得出来");
    assert!(shape(pad + 1) > cap, "越一字节 MUST 越界");
    let sized = |pad: usize| {
        let mut probe = budget_entry("a", "request-pad", 1);
        probe.request_id = "r".repeat(pad);
        probe
    };
    assert!(crate::budget::encode(&sized(pad)).is_ok());
    refusal(
        crate::budget::encode(&sized(pad + 1)).unwrap_err(),
        "oversized ledger entry",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );

    // ---- ② 走得到的最大条目 ----
    let long = "s".repeat(256);
    // 会话得真的存在：`reserve_usage` 先查 `sessions` 表，再谈预算。
    storage
        .create_session(&op(), "test-workspace", "operator", &long)
        .unwrap();
    // `units` 的每一个键都 MUST 在三处限额（global / session / provider）里声明过，
    // 否则是 `UnknownUnit`——所以「把条目顶满」和「把限额顶满」是同一件事。
    let keys: Vec<String> = (0..32)
        .map(|index| {
            if index == 0 {
                "requests".to_string()
            } else {
                format!("unit_{index:0>59}")
            }
        })
        .collect();
    let mut limits = budget_limits();
    limits.sessions = [(long.clone(), limits.sessions["a"].clone())].into();
    limits.providers = [(long.clone(), limits.providers["p"].clone())].into();
    for ceiling in std::iter::once(&mut limits.global)
        .chain(limits.sessions.values_mut())
        .chain(limits.providers.values_mut())
    {
        ceiling.units = keys.iter().map(|key| (key.clone(), u64::MAX)).collect();
    }
    let mut maximal = budget_entry("x", "x", 1);
    maximal.session_id = long.clone();
    maximal.provider_ref = long.clone();
    maximal.request_id = long.clone();
    {
        let estimate = maximal.estimate.as_mut().unwrap();
        estimate.currency = "CNY".into();
        estimate.price_revision = long.clone();
        estimate.units = keys.iter().map(|key| (key.clone(), 1)).collect();
    }
    // 这份夹具把校验允许的每一格都顶满了；再加长一格就会被 `counters` / `id` 拒绝。
    assert_eq!(maximal.session_id.len(), 256);
    assert_eq!(maximal.estimate.as_ref().unwrap().units.len(), 32);
    assert_eq!(
        maximal
            .estimate
            .as_ref()
            .unwrap()
            .units
            .keys()
            .next_back()
            .unwrap()
            .len(),
        64
    );
    storage
        .reserve_usage(&op(), "operator", limits, maximal.clone())
        .unwrap();
    let stored: String = storage
        .conn
        .query_row(
            "SELECT estimate_json FROM usage_ledger WHERE workspace_id='test-workspace' \
             AND request_id=?1",
            params![maximal.request_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        stored.len() <= crate::budget::MAX_STORED_LEDGER_BYTES,
        "被认可的条目 MUST 过得了读回侧的文本闸门"
    );
    // 会过期的断言：这句陈述在「某个字段能长过 256 字符」的那天失效，届时这条会红。
    assert!(
        maximal.encode_to_vec().len() < cap / 4,
        "今天被认可的条目最多几 KB ⇒ 写入上限是**防御性**的，不是路径可达的；\
         若它开始逼近上限，说明校验放松了，线上口径 MUST 重新审"
    );
    // 读回侧的十六进制窗口正是写入上限的两倍：同一个数的两侧，不是一个巧合。
    assert_eq!(crate::budget::MAX_STORED_LEDGER_HEX, cap * 2);
}

/// 存进库里的字节：与 `budget::encode` 同形，多一个解码器会忽略的填充成员。
fn stored_ledger_text(hex: &str, pad: usize) -> String {
    format!(
        r#"{{"format":"nai.v1.UsageLedgerEntry","protobuf_hex":"{hex}","pad":"{}"}}"#,
        "x".repeat(pad)
    )
}

/// R08 ⑩ + ⑪：读回侧的两道闸门。⑪ 那条 `if` 里有**三个**条件（奇长度 / 超长 /
/// 非十六进制），逐个给夹具；而「长度恰在界上」那一侧必须证明自己**没被**这一条挡下
/// ——它继续走到下一条具名拒绝（protobuf 解不开），这才是「边界在哪一侧」。
#[test]
fn the_stored_ledger_gates_are_separate_and_attributable() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    grants_budget(&mut storage);
    storage
        .create_session(&op(), "test-workspace", "operator", "a")
        .unwrap();

    storage
        .reserve_usage(
            &op(),
            "operator",
            budget_limits(),
            budget_entry("a", "request-stored", 1),
        )
        .unwrap();
    let stored: String = storage
        .conn
        .query_row(
            "SELECT estimate_json FROM usage_ledger WHERE workspace_id='test-workspace' \
             AND request_id='request-stored'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let hex = serde_json::from_str::<serde_json::Value>(&stored).unwrap()["protobuf_hex"]
        .as_str()
        .unwrap()
        .to_string();
    let rewrite = |storage: &Store, text: &str| {
        storage
            .conn
            .execute(
                "UPDATE usage_ledger SET estimate_json=?1 WHERE workspace_id='test-workspace' \
                 AND request_id='request-stored'",
                params![text],
            )
            .unwrap()
    };
    let read = |storage: &Store| {
        storage
            .budget_book(&op(), "test-workspace", "operator", budget_limits())
            .map(|_| ())
    };

    // ---- ⑩ 文本长度：恰在界上必须读得回来 ----
    let text_cap = crate::budget::MAX_STORED_LEDGER_BYTES;
    let overhead = stored_ledger_text(&hex, 0).len();
    rewrite(&storage, &stored_ledger_text(&hex, text_cap - overhead));
    assert_eq!(
        stored_ledger_text(&hex, text_cap - overhead).len(),
        text_cap
    );
    read(&storage).unwrap();
    rewrite(&storage, &stored_ledger_text(&hex, text_cap - overhead + 1));
    refusal(
        read(&storage).unwrap_err(),
        "oversized stored ledger",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );

    // ---- ⑪ 十六进制窗口 ----
    let hex_cap = crate::budget::MAX_STORED_LEDGER_HEX;
    // 恰在界上：这一条闸门 MUST NOT 开火，于是拒绝换成了下一条（protobuf 解不开）。
    // 先证明夹具**够得到**这一层：文本长度没被 ⑩ 挡下。
    let at_cap = stored_ledger_text(&"f".repeat(hex_cap), 0);
    assert!(at_cap.len() < text_cap, "夹具 MUST 先过 ⑩ 才谈得上 ⑪");
    rewrite(&storage, &at_cap);
    refusal(
        read(&storage).unwrap_err(),
        "invalid stored ledger protobuf",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );
    // 越界一字节（偶数长度，所以只可能是长度这一条）
    let over = stored_ledger_text(&"f".repeat(hex_cap + 2), 0);
    assert!(over.len() < text_cap);
    rewrite(&storage, &over);
    refusal(
        read(&storage).unwrap_err(),
        "invalid stored ledger bytes",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );
    // 奇长度：同一个 `if` 的第一条，单独举证
    rewrite(&storage, &stored_ledger_text("fffff", 0));
    refusal(
        read(&storage).unwrap_err(),
        "invalid stored ledger bytes",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );
    // 非十六进制：同一个 `if` 的第三条，单独举证（长度偶数且远在界内）
    rewrite(&storage, &stored_ledger_text("zzzz", 0));
    refusal(
        read(&storage).unwrap_err(),
        "invalid stored ledger bytes",
        nai_contracts::v1::ErrorCode::InvalidArgument,
    );
    // 复原：同一行重新可读，于是上面每一处的拒绝都可归因到那一次改写。
    rewrite(&storage, &stored);
    read(&storage).unwrap();
}

/// R08 ①：撤销批次的上限。这条是本组里唯一需要**真实路径**的——表里的不重复
/// (document, revision) 只能由「重新导入（铸新版本）→ 暂存快照」产生，一次一对。
/// 凑到界上是 16 轮 × 256 份文档 = 4096 对；界上那一次必须**过得去**，
/// 第 4097 对才被拒。
#[test]
fn the_revocation_batch_bound_counts_distinct_document_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    let cap = crate::knowledge::MAX_REVOCATION_REVISIONS;
    let width = crate::publication::MAX_SNAPSHOT_DOCUMENTS;
    let documents: Vec<String> = (0..width).map(|index| format!("doc-{index}")).collect();
    let at_cap: Vec<&str> = documents.iter().map(|value| value.as_str()).collect();

    for round in 0..cap / width {
        for document in &documents {
            let body = format!("{document} 第{round}版正文");
            storage
                .import_document(&op(), "operator", &revised_document(document, &body))
                .unwrap();
        }
        stage(&mut storage, &format!("snap-r{round}"), &at_cap).unwrap();
    }
    assert_eq!(
        distinct_revisions(&storage),
        cap as i64,
        "夹具必须**恰在界上**"
    );

    // 界上：放行。返回值是这次撤销波及的活块数（界上每份文档正好一个块）。
    let revoked = storage
        .revoke_knowledge(&op(), "operator", "test-workspace", None, None)
        .unwrap();
    assert_eq!(revoked, width, "恰在界上的批次 MUST 被放行");
    let tombstones = count(
        &storage,
        "SELECT COUNT(*) FROM knowledge_document_revocations WHERE workspace_id='test-workspace'",
    );
    assert_eq!(tombstones, cap as i64);

    // 导入会把 `documents.status` 复位，所以同一份文档还能铸出第 4097 个版本。
    storage
        .import_document(
            &op(),
            "operator",
            &revised_document("doc-0", "doc-0 另起一版正文"),
        )
        .unwrap();
    stage(&mut storage, "snap-over", &["doc-0"]).unwrap();
    assert_eq!(distinct_revisions(&storage), cap as i64 + 1);

    let error = storage
        .revoke_knowledge(&op(), "operator", "test-workspace", None, None)
        .unwrap_err();
    refusal(
        error,
        "revocation revision bound",
        nai_contracts::v1::ErrorCode::ResourceExhausted,
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM knowledge_document_revocations WHERE workspace_id='test-workspace'",
        ),
        cap as i64,
        "被拒的批次 MUST NOT 落下任何撤销记录"
    );
}
