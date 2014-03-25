use std::io;
use std::io::IoResult;

pub use common::{LineTerminator, Row, LF, CRLF, INVALID_LINE_ENDING};

#[deriving(Eq, Clone)]
pub enum Justification {
    Left,
    Right,
}

#[deriving(Eq, Clone)]
pub enum LineEnding {
    Nothing,
    FixedWidth(uint),
    Newline(LineTerminator),
}

#[deriving(Eq, Clone)]
pub struct ColumnConfig {
    width: uint,
    pad_with: char,
    justification: Justification,
}

#[deriving(Eq, Clone)]
pub struct Config {
    columns: Vec<ColumnConfig>,
    line_end: LineEnding,
}

struct Columns<'a, R> {
    reader: &'a mut R,
    config: Config,
    column: uint,
    pos: uint,
    done: bool,
}

impl<'a, R: Buffer> Columns<'a, R> {
    fn read_char(&mut self) -> IoResult<char> {
        self.pos += 1;
        self.reader.read_char()
    }

    fn read_str(&mut self, len: uint) -> IoResult<~str> {
        let mut s = ~"";
        for _ in range(0, len) {
            match self.read_char() {
                Ok(ch) => s.push_char(ch),
                Err(err) => return Err(err)
            }
        }
        Ok(s)
    }

    fn read_column(&mut self, config: ColumnConfig) -> IoResult<~str> {
        match self.read_str(config.width) {
            Ok(col) => {
                let trimmed = if config.justification == Left {
                    col.trim_right_chars(&config.pad_with)
                } else {
                    col.trim_left_chars(&config.pad_with)
                };
                Ok(trimmed.to_owned())
            }
            Err(err) => Err(err)
        }
    }

    fn read_line_ending(&mut self) -> IoResult<()> {
        match self.config.line_end {
            Nothing => Ok(()),
            FixedWidth(w) => self.read_fixed_width(w),
            Newline(lt) => self.read_newline(lt)
        }
    }

    fn read_newline(&mut self, lt: LineTerminator) -> IoResult<()> {
        let expected = lt.as_str();
        let curr_pos = self.pos;
        let actual = match self.read_str(expected.len()) {
            Ok(x) => x,
            Err(ref err) if err.kind == io::EndOfFile && curr_pos + 1 == self.pos => {
                return Ok(())
            }
            Err(err) => return Err(err)
        };
        if expected == actual.as_slice() {
            Ok(())
        } else {
            Err(INVALID_LINE_ENDING.clone())
        }
    }

    fn read_fixed_width(&mut self, width: uint) -> IoResult<()> {
        try!(self.read_str(width - self.pos));
        Ok(())
    }
}

impl<'a, R: Buffer> Iterator<IoResult<~str>> for Columns<'a, R> {
    fn next(&mut self) -> Option<IoResult<~str>> {
        if self.done {
            return None
        }
        let cfg = self.config.columns.get(self.column).clone();
        self.column += 1;
        let col = match self.read_column(cfg) {
            Ok(col) => Ok(col),
            Err(err) => {
                self.done = true;
                if err.kind == io::EndOfFile && self.pos == 1 {
                    return None
                } else {
                    Err(err)
                }
            }
        };
        if self.column == self.config.columns.len() {
            match self.read_line_ending() {
                Ok(()) => (),
                Err(err) => {
                    self.done = true;
                    return Some(Err(err))
                }
            }
        }
        self.done = self.column >= self.config.columns.len();
        Some(col)
    }
}

pub fn read_row<R: Buffer>(config: Config, reader: &mut R) -> IoResult<Row> {
    let mut cols = Columns {
        reader: reader,
        config: config,
        column: 0,
        pos: 0,
        done: false
    };
    let mut row = Vec::new();
    for col in cols {
        match col {
            Ok(c) => row.push(c),
            Err(err) => return Err(err)
        }
    }
    Ok(row)
}

pub struct Rows<R> {
    priv reader: R,
    priv config: Config,
    priv done: bool,
}

impl<R: Buffer> Iterator<IoResult<Row>> for Rows<R> {
    fn next(&mut self) -> Option<IoResult<Row>> {
        if self.done {
            return None
        }
        match read_row(self.config.clone(), &mut self.reader) {
            Ok(row) => {
                if row.len() == 0 {
                    self.done = true;
                    return None
                }
                Some(Ok(row))
            }
            Err(err) => {
                self.done = true;
                Some(Err(err))
            }
        }
    }
}

pub fn read_rows<R: Buffer>(config: Config, reader: R) -> Rows<R> {
    Rows {
        reader: reader,
        config: config,
        done: false
    }
}

#[cfg(test)]
mod test {
    use std::io;
    use std::io::{IoResult, IoError};

    use super::{Config, ColumnConfig, Left, Right, Row, CRLF, Newline, FixedWidth, LF, Nothing};
    use super::{read_row, read_rows, INVALID_LINE_ENDING};

    fn assert_colmatch(cfg: Config, row: &str, cols: IoResult<Row>) {
        let mut reader = io::BufReader::new(row.as_bytes());
        let result = {
            read_row(cfg, &mut reader)
        };
        match reader.read_char() {
            Ok(_) => fail!("Should consume all input"),
            _ => ()
        }
        assert_eq!(cols, result)
    }

    static COLUMN_1: ColumnConfig = ColumnConfig {
        width: 3,
        pad_with: ' ',
        justification: Right
    };

    static COLUMN_2: ColumnConfig = ColumnConfig {
        width: 1,
        pad_with: '#',
        justification: Right
    };

    static COLUMN_3: ColumnConfig = ColumnConfig {
        width: 5,
        pad_with: '-',
        justification: Left
    };

    static COLUMN_ZERO: ColumnConfig = ColumnConfig {
        width: 0,
        pad_with: ' ',
        justification: Left
    };

    #[test]
    fn read_fixed_empty() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg, "", Ok(vec!()));
    }

    #[test]
    fn read_fixed_column_no_padding() {
        let cfg = Config {
            columns: vec!(COLUMN_1),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg, "aaa", Ok(vec!(~"aaa")));
    }

    #[test]
    fn read_fixed_columns_no_padding() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg, "aaabccccc", Ok(vec!(~"aaa", ~"b", ~"ccccc")));
    }

    #[test]
    fn read_fixed_with_zero_length_column() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_ZERO, COLUMN_3),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg, "aaaccccc", Ok(vec!(~"aaa", ~"", ~"ccccc")));
    }

    #[test]
    fn read_fixed_columns_with_padding() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg, "  a#cccc-", Ok(vec!(~"a", ~"", ~"cccc")));
    }

    #[test]
    fn read_fixed_columns_with_newline_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: Newline(CRLF)
        };
        assert_colmatch(Config {line_end: Newline(LF), ..cfg.clone()}, "aaab\n", Ok(vec!(~"aaa", ~"b")));
        assert_colmatch(cfg, "aaab\r\n", Ok(vec!(~"aaa", ~"b")));
    }

    #[test]
    fn read_fixed_columns_with_invalid_newline_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg.clone(), "aaab\r\r", Err(INVALID_LINE_ENDING.clone()));
        assert_colmatch(cfg, "aaab\r", Err(IoError {
            kind: io::EndOfFile,
            desc: "end of file",
            detail: None
        }));
    }

    #[test]
    fn read_fixed_columns_with_fixed_width_length() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: FixedWidth(10)
        };
        assert_colmatch(cfg, "aaab      ", Ok(vec!(~"aaa", ~"b")));
    }

    #[test]
    fn read_fixed_columns_error_on_not_enough_data() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: Newline(LF)
        };
        assert_colmatch(cfg, "aab", Err(IoError {
            kind: io::EndOfFile,
            desc: "end of file",
            detail: None
        }));
    }

    fn assert_rowmatch(config: Config, s: &str, ex: Vec<IoResult<Row>>) {
        let reader = io::BufReader::new(s.as_bytes());
        let rows: Vec<IoResult<Row>> = read_rows(config, reader).collect();
        for (row, exrow) in rows.iter().zip(ex.iter()) {
            assert_eq!(row, exrow);
        }
        if rows.len() < ex.len() {
            fail!("Missing rows: {}", ex.slice_from(rows.len()))
        } else if rows.len() > ex.len() {
            fail!("Unexpected rows: {}", rows.slice_from(ex.len()))
        }
    }

    #[test]
    fn read_lines_with_fixed_columns_and_newline_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Newline(CRLF)
        };
        assert_rowmatch(cfg, " aabccc--\r\n  a#-----", vec!(Ok(vec!(~"aa", ~"b", ~"ccc")), Ok(vec!(~"a", ~"", ~""))));
    }

    #[test]
    fn read_lines_with_fixed_columns_and_fixed_width_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: FixedWidth(10)
        };
        assert_rowmatch(cfg, " aabccc--   a#----- ", vec!(Ok(vec!(~"aa", ~"b", ~"ccc")), Ok(vec!(~"a", ~"", ~""))));
    }

    #[test]
    fn read_lines_with_fixed_columns_and_no_line_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Nothing
        };
        assert_rowmatch(cfg, " aabccc--  a#-----", vec!(Ok(vec!(~"aa", ~"b", ~"ccc")), Ok(vec!(~"a", ~"", ~""))));
    }
}
