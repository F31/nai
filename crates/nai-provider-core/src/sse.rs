/// SSE decoder over a raw byte stream. SSE is UTF-8; `0x0A` never occurs inside
/// a multi-byte character, so splitting on newlines is safe across chunk
/// boundaries. Multi-line `data:` fields are joined with a newline per SSE spec.
#[derive(Debug, Default)]
pub struct SseDecoder {
    line: Vec<u8>,
    data: Vec<u8>,
    done: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub data: String,
    pub done: bool,
}
impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn done(&self) -> bool {
        self.done
    }
    /// Feed a chunk of bytes; returns events terminated by a blank line.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        let mut events = Vec::new();
        for &byte in bytes {
            if self.done {
                return events;
            }
            if byte == b'\n' {
                events.extend(self.end_of_line());
            } else {
                self.line.push(byte);
            }
        }
        events
    }
    fn end_of_line(&mut self) -> Vec<SseEvent> {
        let mut line = std::mem::take(&mut self.line);
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let mut events = Vec::new();
        if line.is_empty() {
            if !self.data.is_empty() {
                let text = String::from_utf8_lossy(&self.data).into_owned();
                let done = text.trim() == "[DONE]";
                events.push(SseEvent { data: text, done });
                self.data.clear();
                if done {
                    self.done = true;
                }
            }
        } else if let Some(rest) = line.strip_prefix(b"data:") {
            let value = rest.strip_prefix(b" ").unwrap_or(rest);
            if !self.data.is_empty() {
                self.data.push(b'\n');
            }
            self.data.extend_from_slice(value);
        }
        events
    }
    /// Flush any unterminated final `data:` event on EOF.
    pub fn finish(&mut self) -> Vec<SseEvent> {
        self.push(b"\n\n")
    }
}
