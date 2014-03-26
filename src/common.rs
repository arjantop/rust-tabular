use std::io;
use std::io::IoError;

/// Newline terminator
#[deriving(Eq, Clone)]
pub enum LineTerminator {
    /// Line terminator '\n'
    LF,
    /// Line terminator '\r\n'
    CRLF
}

impl LineTerminator {
    pub fn as_str(&self) -> &'static str {
        match *self {
            LF => "\n",
            CRLF => "\r\n"
        }
    }

    pub fn is_beginning(&self, ch: char) -> bool {
        match *self {
            LF => ch == '\n',
            CRLF => ch == '\r'
        }
    }
}

/// One row with columns
pub type Row = Vec<~str>;

pub static INVALID_LINE_ENDING: IoError = IoError {
    kind: io::InvalidInput,
    desc: "Invalid line ending",
    detail: None
};
