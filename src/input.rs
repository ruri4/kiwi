#[derive(Debug, Default)]
pub struct Composer {
    text: String,
    cursor: usize,
}

impl Composer {
    pub fn from_text(text: String) -> Self {
        let cursor = text.len();
        Self { text, cursor }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn take(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }

    pub fn insert_char(&mut self, character: char) {
        self.text.insert(self.cursor, character);
        self.cursor += character.len_utf8();
    }

    pub fn insert_str(&mut self, value: &str) {
        self.text.insert_str(self.cursor, value);
        self.cursor += value.len();
    }

    pub fn left(&mut self) {
        let Some(before) = self.text.get(..self.cursor) else {
            return;
        };
        if let Some((index, _)) = before.char_indices().next_back() {
            self.cursor = index;
        }
    }

    pub fn right(&mut self) {
        let Some(after) = self.text.get(self.cursor..) else {
            return;
        };
        if let Some(character) = after.chars().next() {
            self.cursor += character.len_utf8();
        }
    }

    pub fn home(&mut self) {
        if let Some(before) = self.text.get(..self.cursor) {
            self.cursor = before.rfind('\n').map_or(0, |index| index + 1);
        }
    }

    pub fn end(&mut self) {
        if let Some(after) = self.text.get(self.cursor..) {
            let offset = after.find('\n').unwrap_or(after.len());
            self.cursor += offset;
        }
    }

    pub fn backspace(&mut self) {
        let previous = self
            .text
            .get(..self.cursor)
            .and_then(|before| before.char_indices().next_back())
            .map(|(index, _)| index);
        if let Some(index) = previous {
            self.text.drain(index..self.cursor);
            self.cursor = index;
        }
    }

    pub fn delete(&mut self) {
        let Some(after) = self.text.get(self.cursor..) else {
            return;
        };
        if let Some(character) = after.chars().next() {
            self.text
                .drain(self.cursor..self.cursor + character.len_utf8());
        }
    }
}
