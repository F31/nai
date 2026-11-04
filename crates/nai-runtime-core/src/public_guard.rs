//! Exact physical specs or bounded, operator-verified local software features. No execution authority.
use crate::{
    foundation::{Operation, Result, uuid},
    public_media::{resource_axes, sha},
    publication,
};
use nai_contracts::{
    public_caption_hash, public_guard_hash,
    workbench::{
        PublicGuardObject, PublicGuardPolicy, PublicGuardReason as Reason, PublicGuardReview,
        WorkbenchError as Error,
    },
};
use serde_json::Value;

pub fn policy_id() -> &'static str {
    PublicGuardPolicy::ExactSpecsAndSoftwareV5.as_str_name()
}
#[cfg(test)]
mod tests;
pub struct ReviewInput<'a> {
    pub documents: [&'a Value; 4],
    pub cue_id: &'a str,
    pub faq_id: Option<&'a str>,
    pub text: &'a str,
    pub now: i64,
    pub source_valid_until: i64,
    pub output_id: &'a str,
}
fn policy(op: &Operation, x: &ReviewInput<'_>) -> Result<Reason> {
    let product = x.documents[1];
    let cue = x.documents[0]["cues"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .find(|c| c["id"] == x.cue_id)
        .ok_or(Error::NotFound)?;
    let track = if let Some(id) = x.faq_id {
        product["faq"]
            .as_array()
            .ok_or(Error::IntegrityFailed)?
            .iter()
            .find(|f| f["id"] == id)
            .ok_or(Error::NotFound)?
    } else {
        product["talk_tracks"]
            .as_array()
            .ok_or(Error::IntegrityFailed)?
            .iter()
            .find(|t| t["id"] == cue["talk_track_id"])
            .ok_or(Error::IntegrityFailed)?
    };
    let track_text = track["text"].as_str().ok_or(Error::IntegrityFailed)?;
    let selected = track_text == x.text
        || crate::narration::sentences(track_text)
            .is_ok_and(|parts| parts.iter().any(|s| s == x.text));
    if !selected {
        return Ok(Reason::UnsupportedSpeech);
    }
    for word in [
        "治疗",
        "疗效",
        "根治",
        "治愈",
        "医疗",
        "药",
        "疾病",
        "保健",
        "稳赚",
        "保证收益",
        "保本",
        "返现",
        "优惠",
        "折扣",
        "下单",
        "领券",
        "元",
        "免费",
        "永久",
        "零风险",
        "最",
        "第一",
    ] {
        if track_text.contains(word) {
            return Ok(Reason::RiskyContent);
        }
    }
    for rule in product["prohibitions"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
    {
        op.check()?;
        let rule = rule.as_str().ok_or(Error::IntegrityFailed)?;
        let phrase = [
            "禁止：",
            "禁止:",
            "不得宣称",
            "不得承诺",
            "不得提及",
            "不得声称",
        ]
        .iter()
        .find_map(|prefix| rule.strip_prefix(prefix))
        .map(str::trim);
        let Some(phrase) = phrase else {
            return Ok(Reason::UnsupportedRule);
        };
        if phrase.is_empty()
            || phrase.chars().count() > 64
            || !phrase.chars().all(char::is_alphanumeric)
            || [
                "除非", "例外", "如果", "除外", "允许", "但是", "否则", "只要", "条件", "可以",
                "或者", "但",
            ]
            .iter()
            .any(|word| phrase.contains(word))
        {
            return Ok(Reason::UnsupportedRule);
        }
        if track_text.contains(phrase) {
            return Ok(Reason::ProhibitedPhrase);
        }
    }
    let facts = product["facts"].as_array().ok_or(Error::IntegrityFailed)?;
    let ids = track["fact_ids"].as_array().ok_or(Error::IntegrityFailed)?;
    if ids.is_empty() || ids.len() > 8 {
        return Ok(Reason::UnsupportedSpeech);
    }
    let mut exact = String::new();
    let software = ids.iter().all(|id| {
        facts
            .iter()
            .any(|f| f["id"] == *id && f["value_type"] == "text")
    });
    for id in ids {
        op.check()?;
        let f = facts
            .iter()
            .find(|f| f["id"] == *id)
            .ok_or(Error::IntegrityFailed)?;
        let label = f["label"].as_str().ok_or(Error::IntegrityFailed)?;
        let unit = f["unit"].as_str().unwrap_or("");
        let supported = [
            ("容量", "毫升"),
            ("容量", "升"),
            ("容量", "mL"),
            ("容量", "L"),
            ("净含量", "毫升"),
            ("重量", "克"),
            ("净重", "克"),
            ("重量", "千克"),
            ("净重", "千克"),
            ("长度", "厘米"),
            ("宽度", "厘米"),
            ("高度", "厘米"),
            ("直径", "毫米"),
        ];
        let value = f["value"].as_str().unwrap_or("");
        if software {
            if label != "本机功能"
                || !unit.is_empty()
                || f.get("currency").is_some()
                || f["conditions"] != "本机工作台"
                || f["authority"] != "owner_confirmed"
                || ![
                    "手动提词",
                    "节目编排",
                    "资料与FAQ管理",
                    "基础二维形象与字幕",
                    "真人麦克风接管",
                    "问题复盘与下场草稿",
                ]
                .contains(&value)
            {
                return Ok(Reason::UnsupportedSpeech);
            }
            exact.push_str(&format!("本机工作台支持{value}。"));
            continue;
        }
        if f["value_type"] != "decimal"
            || !supported.contains(&(label, unit))
            || value.is_empty()
            || value.len() > 16
            || value.starts_with('.')
            || value.ends_with('.')
            || value.bytes().filter(|b| *b == b'.').count() > 1
            || !value.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            || !value.bytes().any(|b| (b'1'..=b'9').contains(&b))
        {
            return Ok(Reason::UnsupportedSpeech);
        }
        let conditions = f["conditions"].as_str().ok_or(Error::IntegrityFailed)?;
        if ![
            "",
            "标准款",
            "大号",
            "中号",
            "小号",
            "大容量款",
            "基础款",
            "经典款",
            "白色款",
            "黑色款",
            "蓝色款",
            "红色款",
            "绿色款",
        ]
        .contains(&conditions)
        {
            return Ok(Reason::UnsupportedSpeech);
        }
        exact.push_str(&format!("{conditions}{label}为{value}{unit}。"));
    }
    let intro = format!(
        "这是一款{}，{exact}",
        product["name"].as_str().ok_or(Error::IntegrityFailed)?
    );
    let safe_name = [
        "便携水杯",
        "水杯",
        "保温杯",
        "收纳盒",
        "背包",
        "量杯",
        "玻璃杯",
        "不锈钢水杯",
        "储物箱",
        "旅行箱",
        "桌子",
        "椅子",
        "水壶",
        "保温壶",
        "杯子",
    ]
    .contains(&product["name"].as_str().unwrap_or(""));
    Ok(if software && track_text == exact {
        Reason::ExactFeaturePassed
    } else if !software && (track_text == exact || safe_name && track_text == intro) {
        Reason::ExactSpecPassed
    } else {
        Reason::UnsupportedSpeech
    })
}
/// Source-sample duration plus one execution lease of start/submit margin.
/// Does not extend any source deadline or network operation.
pub fn required_playback_window_ms(
    audio: &nai_contracts::workbench::PublicMediaAudio,
) -> Result<u64> {
    if audio.sample_rate != 24000
        || audio.channels != 1
        || audio.sample_count == 0
        || audio.sample_count > 480000
    {
        return Err(Error::InvalidArgument);
    }
    Ok((audio.sample_count * 1000).div_ceil(u64::from(audio.sample_rate)) + 500)
}
pub fn review(
    op: &Operation,
    object: PublicGuardObject,
    x: &ReviewInput<'_>,
) -> Result<PublicGuardReview> {
    op.check()?;
    crate::tts::characters(x.text)?;
    if object.protocol_major != 1
        || object.policy_id != policy_id()
        || object.output_target_id != x.output_id
        || object.epoch == 0
    {
        return Err(Error::InvalidArgument);
    }
    for id in [
        &object.workspace_id,
        &object.session_id,
        &object.preview_id,
        &object.task_id,
        &object.snapshot_id,
        &object.output_target_id,
    ] {
        uuid(id)?;
    }
    for hash in [
        &object.preview_sha256,
        &object.task_sha256,
        &object.snapshot_sha256,
        &object.facts_sha256,
        &object.caption_sha256,
    ] {
        if !sha(hash) {
            return Err(Error::InvalidArgument);
        }
    }
    let text_sha = public_caption_hash(x.text);
    if text_sha != object.caption_sha256 {
        return Err(Error::IntegrityFailed);
    }
    resource_axes(
        object.audio.as_ref().ok_or(Error::CapabilityUnavailable)?,
        object.visual.as_ref().ok_or(Error::CapabilityUnavailable)?,
    )?;
    let closure = publication::closure(op, x.documents, x.now)?;
    let until = closure
        .valid_until_ms
        .unwrap_or(x.source_valid_until)
        .min(x.source_valid_until)
        .min(x.now.checked_add(30000).ok_or(Error::DeadlineExceeded)?);
    if x.now < 0 || until <= x.now {
        return Err(Error::DeadlineExceeded);
    }
    let reason = policy(op, x)?;
    op.check()?;
    Ok(PublicGuardReview {
        object_sha256: public_guard_hash(&object),
        object: Some(object),
        policy_passed: matches!(reason, Reason::ExactSpecPassed | Reason::ExactFeaturePassed),
        reason: reason as i32,
        checked_at_ms: x.now as u64,
        valid_until_ms: until as u64,
    })
}
