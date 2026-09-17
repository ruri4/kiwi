#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    User,
    Assistant,
    Error,
}

impl Role {
    pub fn api_name(self) -> Option<&'static str> {
        match self {
            Self::User => Some("user"),
            Self::Assistant => Some("assistant"),
            Self::Error => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::User => "you",
            Self::Assistant => "model",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: String,
}
