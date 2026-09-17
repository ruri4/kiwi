use anyhow::{Context, Result, bail};
use serde_json::Value;

#[derive(Debug, Eq, PartialEq)]
pub enum SseItem {
    Content(String),
    Reasoning(String),
    Done,
}

#[derive(Debug, Default)]
pub struct SseDecoder {
    pending: Vec<u8>,
    data_lines: Vec<String>,
    done: bool,
}

impl SseDecoder {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<SseItem>> {
        if self.done {
            return Ok(Vec::new());
        }
        self.pending.extend_from_slice(bytes);
        let mut items = Vec::new();

        while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut line = self.pending.drain(..=newline).collect::<Vec<_>>();
            line.truncate(line.len().saturating_sub(1));
            if line.last() == Some(&b'\r') {
                line.truncate(line.len().saturating_sub(1));
            }
            self.process_line(&line, &mut items)?;
            if self.done {
                self.pending.clear();
                break;
            }
        }
        Ok(items)
    }

    pub fn finish(&mut self) -> Result<Vec<SseItem>> {
        if self.done {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        if !self.pending.is_empty() {
            let mut line = std::mem::take(&mut self.pending);
            if line.last() == Some(&b'\r') {
                line.truncate(line.len().saturating_sub(1));
            }
            self.process_line(&line, &mut items)?;
        }
        self.dispatch(&mut items)?;
        Ok(items)
    }

    fn process_line(&mut self, line: &[u8], items: &mut Vec<SseItem>) -> Result<()> {
        let line = std::str::from_utf8(line).context("SSE stream contained invalid UTF-8")?;
        if line.is_empty() {
            return self.dispatch(items);
        }
        if line.starts_with(':') {
            return Ok(());
        }
        if let Some(value) = line.strip_prefix("data:") {
            let value = value.strip_prefix(' ').unwrap_or(value);
            self.data_lines.push(value.to_owned());
        }
        Ok(())
    }

    fn dispatch(&mut self, items: &mut Vec<SseItem>) -> Result<()> {
        if self.data_lines.is_empty() {
            return Ok(());
        }
        let data = self.data_lines.join("\n");
        self.data_lines.clear();
        if data.trim().is_empty() {
            return Ok(());
        }
        if data.trim() == "[DONE]" {
            self.done = true;
            items.push(SseItem::Done);
            return Ok(());
        }

        let value: Value = serde_json::from_str(&data).with_context(|| {
            let preview: String = data.chars().take(160).collect();
            format!("malformed SSE data JSON: {preview}")
        })?;
        let reasoning_content = optional_string(
            value.pointer("/choices/0/delta/reasoning_content"),
            "reasoning_content",
        )?;
        let reasoning = optional_string(value.pointer("/choices/0/delta/reasoning"), "reasoning")?;
        if let Some(reasoning) = reasoning_content.or(reasoning) {
            items.push(SseItem::Reasoning(reasoning.to_owned()));
        }
        if let Some(content) =
            optional_string(value.pointer("/choices/0/delta/content"), "content")?
        {
            items.push(SseItem::Content(content.to_owned()));
        }
        Ok(())
    }
}

fn optional_string<'a>(value: Option<&'a Value>, field: &str) -> Result<Option<&'a str>> {
    match value {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => bail!("SSE delta {field} was not a string or null"),
    }
}

#[cfg(test)]
mod tests {
    use super::{SseDecoder, SseItem};
    use anyhow::Result;

    #[test]
    fn tolerates_fragmented_chunks() -> Result<()> {
        let mut decoder = SseDecoder::default();
        assert!(decoder.feed(b"da")?.is_empty());
        assert!(
            decoder
                .feed(b"ta: {\"choices\":[{\"delta\":{\"con")?
                .is_empty()
        );
        let items = decoder.feed(b"tent\":\"hello\"}}]}\n\n")?;
        assert_eq!(items, vec![SseItem::Content("hello".into())]);
        Ok(())
    }

    #[test]
    fn accepts_crlf_and_ignores_keepalives() -> Result<()> {
        let mut decoder = SseDecoder::default();
        let items = decoder.feed(
            b": heartbeat\r\n\r\ndata:\r\n\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\r\n\r\n",
        )?;
        assert_eq!(items, vec![SseItem::Content("ok".into())]);
        Ok(())
    }

    #[test]
    fn streams_both_reasoning_field_names_and_content() -> Result<()> {
        let mut decoder = SseDecoder::default();
        let items = decoder.feed(
            b"data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"first\",\"content\":\"answer\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"reasoning\":\" second\"}}]}\n\n",
        )?;
        assert_eq!(
            items,
            vec![
                SseItem::Reasoning("first".into()),
                SseItem::Content("answer".into()),
                SseItem::Reasoning(" second".into()),
            ]
        );
        Ok(())
    }

    #[test]
    fn prefers_reasoning_content_when_both_are_strings() -> Result<()> {
        let mut decoder = SseDecoder::default();
        let items = decoder.feed(
            b"data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"preferred\",\"reasoning\":\"duplicate\"}}]}\n\n",
        )?;
        assert_eq!(items, vec![SseItem::Reasoning("preferred".into())]);
        Ok(())
    }

    #[test]
    fn malformed_reasoning_types_are_errors() {
        for field in ["reasoning_content", "reasoning"] {
            let mut decoder = SseDecoder::default();
            let event =
                format!("data: {{\"choices\":[{{\"delta\":{{\"{field}\":[\"nope\"]}}}}]}}\n\n");
            let error = decoder.feed(event.as_bytes());
            assert!(error.is_err_and(|value| {
                value
                    .to_string()
                    .contains(&format!("{field} was not a string or null"))
            }));
        }
    }

    #[test]
    fn malformed_json_is_an_error() {
        let mut decoder = SseDecoder::default();
        let error = decoder.feed(b"data: {nope}\n\n");
        assert!(error.is_err_and(|value| value.to_string().contains("malformed SSE data JSON")));
    }

    #[test]
    fn done_stops_later_data() -> Result<()> {
        let mut decoder = SseDecoder::default();
        let items = decoder.feed(
            b"data: [DONE]\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"late\"}}]}\n\n",
        )?;
        assert_eq!(items, vec![SseItem::Done]);
        assert!(decoder.finish()?.is_empty());
        Ok(())
    }

    #[test]
    fn clean_eof_dispatches_last_event() -> Result<()> {
        let mut decoder = SseDecoder::default();
        decoder.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"tail\"}}]}")?;
        assert_eq!(decoder.finish()?, vec![SseItem::Content("tail".into())]);
        Ok(())
    }
}
