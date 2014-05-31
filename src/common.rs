use std::io;
use std::io::IoError;

/// Line terminator
#[deriving(Eq, PartialEq, Clone)]
pub enum LineTerminator {
    /// Line feed ('\n')
    LF,
    /// Carriage return ('\r')
    CR,
    /// CR followed by LF ('\r\n')
    CRLF,
    /// Vertical tab (u000B)
    VT,
    /// Form feed (u000C)
    FF,
    /// Next line (u0085)
    NEL,
    /// Line separator (u2028)
    LS,
    /// Paragraph simulator (u2029)
    PS,
}

impl LineTerminator {
    pub fn as_str(&self) -> &'static str {
        match *self {
            LF => "\n",
            CR => "\r",
            CRLF => "\r\n",
            VT => "\u000B",
            FF => "\u000C",
            NEL => "\u0085",
            LS => "\u2028",
            PS => "\u2029",
        }
    }

    pub fn is_beginning(&self, ch: char) -> bool {
        ch == self.as_str().char_at(0)
    }
}

/// One row with columns
pub type Row = Vec<String>;

pub static INVALID_LINE_ENDING: IoError = IoError {
    kind: io::InvalidInput,
    desc: "Invalid line ending",
    detail: None
};
