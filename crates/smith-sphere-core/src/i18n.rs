//! Interface language and a two-language string picker.
//!
//! The application carries the selected language explicitly and passes it to
//! every function that produces human-readable text, so the whole interface
//! switches at once without any hidden global state.

use serde::{Deserialize, Serialize};

/// The languages the interface speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Lang {
    /// Simplified Chinese.
    #[default]
    Chinese,
    /// English.
    English,
}

impl Lang {
    /// Every language, in menu order.
    pub const ALL: [Lang; 2] = [Lang::Chinese, Lang::English];

    /// Returns the Chinese or English argument for the current language.
    #[must_use]
    pub fn pick<'a>(self, chinese: &'a str, english: &'a str) -> &'a str {
        match self {
            Lang::Chinese => chinese,
            Lang::English => english,
        }
    }

    /// The word for a points count, as in "77 points".
    #[must_use]
    pub fn points_word(self) -> &'static str {
        self.pick("个频点", "points")
    }

    /// The name of the language in its own script, for a language menu.
    #[must_use]
    pub fn endonym(self) -> &'static str {
        match self {
            Lang::Chinese => "中文",
            Lang::English => "English",
        }
    }

    /// Maps a BCP-47 or POSIX locale code such as `zh-CN` or `en_US` to a
    /// language. Returns `None` when the code names neither.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Lang> {
        let lower = code.trim().to_ascii_lowercase();
        if lower.starts_with("zh") {
            Some(Lang::Chinese)
        } else if lower.starts_with("en") {
            Some(Lang::English)
        } else {
            None
        }
    }
}
