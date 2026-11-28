//! Bounded semantic script and one interruptible preparation ticket. No I/O.
use crate::foundation::Operation;
use nai_contracts::workbench::WorkbenchError as Error;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub max_sentence_chars: usize,
    pub max_sentence_samples: u64,
    pub max_sentences: usize,
    pub idle_wait_ms: u32,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            max_sentence_chars: 80,
            max_sentence_samples: 144000,
            max_sentences: 16,
            idle_wait_ms: 4000,
        }
    }
}
impl Policy {
    pub fn validate(&self) -> Result<(), Error> {
        if !(16..=256).contains(&self.max_sentence_chars)
            || !(4800..=240000).contains(&self.max_sentence_samples)
            || !(1..=32).contains(&self.max_sentences)
            || !(1..=10000).contains(&self.idle_wait_ms)
        {
            return Err(Error::InvalidArgument);
        }
        Ok(())
    }
    pub fn split(&self, text: &str) -> Result<Vec<String>, Error> {
        self.validate()?;
        if text.trim().is_empty() || text.chars().count() > 1024 {
            return Err(Error::InvalidArgument);
        }
        let chars: Vec<_> = text.chars().collect();
        let mut output = Vec::new();
        let mut current = String::new();
        let mut boundary = false;
        for (i, &c) in chars.iter().enumerate() {
            if boundary && !matches!(c, '”' | '’' | '」' | '』' | '）' | '】' | '"') {
                if !current.trim().is_empty() {
                    output.push(current.trim().to_owned());
                }
                current.clear();
                boundary = false;
            }
            current.push(c);
            boundary |= matches!(c, '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n')
                || (c == '.'
                    && chars.get(i + 1).is_none_or(|n| n.is_whitespace())
                    && i > 0
                    && !chars[i - 1].is_ascii_digit());
        }
        if !current.trim().is_empty() {
            output.push(current.trim().to_owned());
        }
        if output.len() > self.max_sentences
            || output
                .iter()
                .any(|s| s.chars().count() > self.max_sentence_chars)
        {
            return Err(Error::InvalidArgument);
        }
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Narration,
    Interjection,
}
pub struct Ticket {
    id: u64,
    pub text: String,
    pub kind: Kind,
    pub operation: Operation,
}
struct Active {
    id: u64,
    kind: Kind,
    interruptible: bool,
    operation: Operation,
}
struct State {
    script: Vec<String>,
    cursor: usize,
    next: u64,
    pending: Option<String>,
    active: Option<Active>,
    closed: bool,
}
#[derive(Clone)]
pub struct Plan {
    root: Operation,
    policy: Policy,
    state: Arc<Mutex<State>>,
}
impl Plan {
    pub fn new(text: &str, policy: Policy, root: Operation) -> Result<Self, Error> {
        root.check()?;
        let script = policy.split(text)?;
        Ok(Self {
            root,
            policy,
            state: Arc::new(Mutex::new(State {
                script,
                cursor: 0,
                next: 0,
                pending: None,
                active: None,
                closed: false,
            })),
        })
    }
    pub fn interject(&self, text: &str) -> Result<(), Error> {
        self.root.check()?;
        let parts = self.policy.split(text)?;
        if parts.len() != 1 {
            return Err(Error::InvalidArgument);
        }
        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
        self.root.check()?;
        if s.closed {
            return Err(Error::Cancelled);
        }
        if s.pending.is_some()
            || s.active
                .as_ref()
                .is_some_and(|a| a.kind == Kind::Interjection)
        {
            return Err(Error::ResourceExhausted);
        }
        if s.active.as_ref().is_some_and(|a| a.interruptible) {
            s.active
                .take()
                .ok_or(Error::ProtocolError)?
                .operation
                .cancel();
        }
        s.pending = Some(parts[0].clone());
        Ok(())
    }
    pub fn take(&self, interruptible: bool) -> Result<Option<Ticket>, Error> {
        self.take_for(interruptible, &self.root)
    }
    pub fn take_for(&self, interruptible: bool, root: &Operation) -> Result<Option<Ticket>, Error> {
        self.root.check()?;
        root.check()?;
        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
        if s.active.is_some() {
            return Err(Error::ResourceExhausted);
        }
        let (text, kind) = if let Some(text) = s.pending.take() {
            (text, Kind::Interjection)
        } else if let Some(text) = s.script.get(s.cursor) {
            (text.clone(), Kind::Narration)
        } else {
            return Ok(None);
        };
        let operation = root.model_child(root.remaining_ms()?)?;
        s.next = s.next.checked_add(1).ok_or(Error::ResourceExhausted)?;
        let id = s.next;
        s.active = Some(Active {
            id,
            kind,
            interruptible,
            operation: operation.clone(),
        });
        Ok(Some(Ticket {
            id,
            text,
            kind,
            operation,
        }))
    }
    pub fn check(&self, ticket: &Ticket) -> Result<(), Error> {
        self.root.check()?;
        ticket.operation.check()?;
        let s = self.state.lock().map_err(|_| Error::IoFailed)?;
        if s.active.as_ref().is_none_or(|a| a.id != ticket.id) {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    /// Called only after the device has consumed the last current sentence.
    pub fn close_if_empty(&self) -> Result<bool, Error> {
        self.root.check()?;
        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
        if s.active.is_some() || s.pending.is_some() || s.cursor < s.script.len() {
            return Ok(false);
        }
        s.closed = true;
        Ok(true)
    }
    pub fn commit(&self, ticket: &Ticket) -> Result<(), Error> {
        self.root.check()?;
        ticket.operation.check()?;
        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
        if s.active.as_ref().is_none_or(|a| a.id != ticket.id) {
            return Err(Error::Cancelled);
        }
        if ticket.kind == Kind::Narration {
            s.cursor += 1;
        }
        s.active = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn semantic_boundaries_preserve_decimal_model_and_quotation() {
        let p = Policy::default();
        assert_eq!(
            p.split("容量3.5升，型号A1.2，不是5升。‘好的。’然后讲解！")
                .unwrap(),
            vec!["容量3.5升，型号A1.2，不是5升。", "‘好的。’", "然后讲解！"]
        );
        assert!(p.split(&"长".repeat(81)).is_err());
        assert!(p.split(&"一句。".repeat(17)).is_err());
    }
    #[test]
    fn question_revokes_only_prefetch_and_narration_resumes_at_uncommitted_sentence() {
        let plan = Plan::new(
            "第一句。第二句。",
            Policy::default(),
            Operation::model_task(4000).unwrap(),
        )
        .unwrap();
        let first = plan.take(false).unwrap().unwrap();
        plan.interject("回答。").unwrap();
        plan.commit(&first).unwrap();
        let answer = plan.take(true).unwrap().unwrap();
        assert_eq!(answer.kind, Kind::Interjection);
        plan.commit(&answer).unwrap();
        let old = plan.take(true).unwrap().unwrap();
        assert!(plan.take(true).is_err());
        plan.interject("新回答。").unwrap();
        assert!(plan.commit(&old).is_err());
        assert!(plan.interject("不能排第三个。").is_err());
        let answer = plan.take(true).unwrap().unwrap();
        plan.commit(&answer).unwrap();
        let resumed = plan.take(true).unwrap().unwrap();
        assert_eq!(resumed.text, "第二句。");
        plan.commit(&resumed).unwrap();
        assert!(plan.take(true).unwrap().is_none());
    }
    #[test]
    fn stop_rejects_prepared_and_future_work() {
        let root = Operation::model_task(4000).unwrap();
        let plan = Plan::new("第一句。", Policy::default(), root.clone()).unwrap();
        let ticket = plan.take(true).unwrap().unwrap();
        root.cancel();
        assert!(plan.commit(&ticket).is_err());
        assert!(plan.take(true).is_err());
        assert!(plan.interject("回答。").is_err());
    }
    #[test]
    fn last_sentence_remains_interruptible_until_device_completion_closes_plan() {
        let plan = Plan::new(
            "最后一句。",
            Policy::default(),
            Operation::model_task(4000).unwrap(),
        )
        .unwrap();
        let last = plan.take(false).unwrap().unwrap();
        plan.commit(&last).unwrap();
        assert!(plan.take(true).unwrap().is_none());
        plan.interject("补充回答。").unwrap();
        assert!(!plan.close_if_empty().unwrap());
        let answer = plan.take(true).unwrap().unwrap();
        plan.commit(&answer).unwrap();
        assert!(plan.close_if_empty().unwrap());
        assert!(plan.interject("不能迟到。").is_err());
    }
}
