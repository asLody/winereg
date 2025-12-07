#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    Unknown,
    Win32,
    Win64,
}

impl Default for Architecture {
    fn default() -> Self {
        Architecture::Unknown
    }
}

impl Architecture {
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag.to_ascii_lowercase().as_str() {
            "win32" => Some(Architecture::Win32),
            "win64" => Some(Architecture::Win64),
            _ => None,
        }
    }

    pub fn as_tag(&self) -> Option<&'static str> {
        match self {
            Architecture::Unknown => None,
            Architecture::Win32 => Some("win32"),
            Architecture::Win64 => Some("win64"),
        }
    }
}

