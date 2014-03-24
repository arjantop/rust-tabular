use std::io;
use std::io::{IoResult, IoError};
use std::vec::Vec;

#[deriving(Eq, Show)]
pub enum Escape {
    Double,
    Char(char),
    Disallowed,
}

pub enum LineTerminator {
    LF,
    CRLF
}

pub static CSV: Config = Config {
    delimiter: ',',
    quote_char: '"',
    escape: Double,
    line_terminator: CRLF
};

pub struct Config {
    delimiter: char,
    quote_char: char,
    escape: Escape,
    line_terminator: LineTerminator
}

impl Config {
    fn escape_char(&self) -> Option<char> {
        match self.escape {
            Double => Some(self.quote_char),
            Char(ch) => Some(ch),
            Disallowed => None
        }
    }

    fn is_line_terminator_start(&self, ch: char) -> bool {
        match self.line_terminator {
            LF => ch == '\n',
            CRLF => ch == '\r'
        }
    }
}

struct Columns<'a, R> {
    reader: &'a mut R,
    config: Config,
    row_done: bool,
    column: uint,
    pos: uint
}

impl<'a, R: Buffer> Columns<'a, R> {
    fn read_char(&mut self) -> IoResult<char> {
        let res = self.reader.read_char();
        if res.is_ok() { self.pos += 1; }
        res
    }

    fn quoted_end(&mut self, next: IoResult<char>, res: ~str) -> IoResult<~str> {
        match next {
            Ok(ch) => {
                if ch == self.config.delimiter {
                    Ok(res)
                } else if self.config.is_line_terminator_start(ch) {
                    match self.read_line_terminator() {
                        Ok(()) => Ok(res),
                        Err(err) => Err(err)
                    }
                } else {
                    Err(IoError {
                        kind: io::InvalidInput,
                        desc: "Expecting line terminator or delimiter",
                        detail: None
                    })
                }
            }
            Err(ref err) if err.kind == io::EndOfFile => {
                self.row_done = true;
                Ok(res)
            }
            Err(err) => Err(err)
        }
    }

    fn read_quoted_column(&mut self) -> IoResult<~str> {
        let mut col = ~"";
        loop {
            match self.read_char() {
                Ok(ch) => {
                    if self.config.escape_char() != Some(self.config.quote_char) && Some(ch) == self.config.escape_char() {
                        match self.read_char() {
                            Ok(quote) if quote == self.config.quote_char => col.push_char(quote),
                            _ => return Err(IoError {
                                kind: io::InvalidInput,
                                desc: "Expecting quote char",
                                detail: None
                            })
                        }

                    } else if self.config.escape_char() != Some(self.config.quote_char) && ch == self.config.quote_char {
                        let next = self.read_char();
                        return self.quoted_end(next, col)
                    } else if ch == self.config.quote_char {
                        let next = self.read_char();
                        match next {
                            Ok(next) if next == self.config.quote_char => {
                                col.push_char(next);
                                continue
                            }
                            _ => ()
                        };
                        return self.quoted_end(next, col)
                    } else {
                        col.push_char(ch);
                    }
                }
                Err(err) => return Err(err)
            }
        }
    }

    fn read_line_terminator(&mut self) -> IoResult<()> {
        let res = match self.config.line_terminator {
            LF => Ok(()),
            CRLF => {
                match self.read_char() {
                    Ok('\n') => Ok(()),
                    Ok(_) => Err(IoError {
                        kind: io::InvalidInput,
                        desc: "Invalid line ending",
                        detail: None
                    }),
                    Err(err) => Err(err)
                }
            }
        };
        if res.is_ok() {
            self.row_done = true;
        }
        res
    }

    fn check_eof(&mut self, err: IoError, allow_empty: bool, res: ~str) -> IoResult<~str> {
        if !self.row_done && err.kind == io::EndOfFile && (res.len() > 0 || allow_empty) {
            self.row_done = true;
            Ok(res)
        } else {
            Err(err)
        }
    }

    fn read_unquoted_column(&mut self, mut curr: IoResult<char>) -> IoResult<~str> {
        let mut col = ~"";
        loop {
            match curr {
                Ok(ch) => {
                    if self.config.is_line_terminator_start(ch) {
                        match self.read_line_terminator() {
                            Ok(()) => break,
                            Err(err) => return Err(err)
                        }
                    } else if ch != self.config.delimiter {
                        col.push_char(ch);
                    } else {
                        break
                    }
                    curr = self.read_char();
                }
                Err(err) => return self.check_eof(err, self.column > 0, col)
            }
        }
        Ok(col)
    }

    fn read_column(&mut self) -> IoResult<~str> {
        let res = match self.read_char() {
            Ok(ch) if self.config.quote_char == ch => self.read_quoted_column(),
            res => self.read_unquoted_column(res)
        };
        if res.is_ok() {
            self.column += 1;
        }
        res
    }
}

impl<'a, R: Buffer> Iterator<IoResult<~str>> for Columns<'a, R> {
    fn next(&mut self) -> Option<IoResult<~str>> {
        if self.row_done {
            return None
        }
        match self.read_column() {
            Err(err) => {
                self.row_done = true;
                if self.pos == 0 && err.kind == io::EndOfFile {
                    None
                } else {
                    Some(Err(err))
                }
            }
            res => Some(res)
        }
    }
}

pub type Row = Vec<~str>;

pub fn read_row<R: Buffer>(config: Config, reader: &mut R) -> IoResult<Row> {
    let mut cols = Columns {
        reader: reader,
        config: config,
        row_done: false,
        column: 0,
        pos: 0
    };
    let mut res = Vec::new();
    for col in cols {
        match col {
            Ok(s) => res.push(s),
            Err(err) => return Err(err)
        }
    }
    Ok(res)
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
        match read_row(self.config, &mut self.reader) {
            Ok(row) => {
                self.done = row.len() == 0;
                if self.done {
                    None
                } else {
                    Some(Ok(row))
                }
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
    use std::vec::Vec;

    use super::{Columns, Config, Char, CSV, read_rows, Row, LF};

    fn assert_colmatch(cfg: Config, row: &str, cols: &[IoResult<~str>]) {
        let mut reader = io::BufReader::new(row.as_bytes());
        let mut columns = Columns {reader: &mut reader, config: cfg, row_done: false, column: 0, pos: 0};
        let result: Vec<IoResult<~str>> = columns.collect();
        assert_eq!(cols, result.as_slice())
    }

    static DELIM_PIPE: Config = Config {delimiter: '|', ..CSV};

    static QUOTE_TILDE: Config = Config {quote_char: '~', ..CSV};

    static EOF_ERROR: IoError = IoError {
        kind: io::EndOfFile,
        desc: "end of file",
        detail: None
    };

    static INVALID_LINE_ENDING: IoError = IoError {
        kind: io::InvalidInput,
        desc: "Invalid line ending",
        detail: None
    };

    #[test]
    fn empty_column() {
        assert_colmatch(CSV, "", []);
    }

    #[test]
    fn empty_column_line_end() {
        assert_colmatch(CSV, "\r\n", [Ok(~"")]);
        assert_colmatch(Config {line_terminator: LF, ..CSV}, "\n", [Ok(~"")]);
    }

    #[test]
    fn single_column() {
        assert_colmatch(CSV, "abc", [Ok(~"abc")]);
        assert_colmatch(DELIM_PIPE, "abc", [Ok(~"abc")]);
    }

    #[test]
    fn single_column_line_end() {
        assert_colmatch(CSV, "foo\r\n", [Ok(~"foo")]);
        assert_colmatch(Config {line_terminator: LF, ..CSV}, "foo\n", [Ok(~"foo")]);
    }

    #[test]
    fn single_column_invalid_line_end() {
        assert_colmatch(CSV, "foo\r\r", [Err(INVALID_LINE_ENDING.clone())]);
    }

    #[test]
    fn multi_column() {
        assert_colmatch(CSV, "foo,bar", [Ok(~"foo"), Ok(~"bar")]);
        assert_colmatch(DELIM_PIPE, "foo|bar", [Ok(~"foo"), Ok(~"bar")]);
    }

    #[test]
    fn multi_column_line_end() {
        assert_colmatch(CSV, "foo,bar\r\n", [Ok(~"foo"), Ok(~"bar")]);
        assert_colmatch(DELIM_PIPE, "foo|bar\r\n", [Ok(~"foo"), Ok(~"bar")]);
        assert_colmatch(Config {line_terminator: LF, ..CSV}, "foo,bar\n", [Ok(~"foo"), Ok(~"bar")]);
    }

    #[test]
    fn empty_column_quoted() {
        assert_colmatch(CSV, r#""""#, [Ok(~"")]);
        assert_colmatch(Config {quote_char: '\'', ..CSV}, "''", [Ok(~"")]);
    }

    #[test]
    fn empty_column_quoted_line_end() {
        assert_colmatch(CSV, "\"\"\r\n", [Ok(~"")]);
        assert_colmatch(Config {quote_char: '\'', ..CSV}, "''\r\n", [Ok(~"")]);
        assert_colmatch(Config {line_terminator: LF, ..CSV}, "\"\"\n", [Ok(~"")]);
        assert_colmatch(Config {line_terminator: LF, quote_char: '\'', ..CSV}, "''\n", [Ok(~"")]);
    }

    #[test]
    fn single_column_quoted() {
        assert_colmatch(CSV, r#""abc""#, [Ok(~"abc")]);
        assert_colmatch(QUOTE_TILDE, r#"~abc~"#, [Ok(~"abc")]);
    }

    #[test]
    fn single_column_quoted_with_delim() {
        assert_colmatch(CSV, r#""a,b,c""#, [Ok(~"a,b,c")]);
        assert_colmatch(Config {delimiter: '-', ..QUOTE_TILDE}, r#"~a-b-c~"#, [Ok(~"a-b-c")]);
    }

    #[test]
    fn single_column_quoted_line_end() {
        assert_colmatch(CSV, "\"abc\"\r\n", [Ok(~"abc")]);
        assert_colmatch(QUOTE_TILDE, "~abc~\r\n", [Ok(~"abc")]);
        assert_colmatch(Config {line_terminator: LF, ..QUOTE_TILDE}, "~abc~\n", [Ok(~"abc")]);
    }

    #[test]
    fn single_column_quoted_invalid_line_end() {
        assert_colmatch(CSV, "\"abc\"\r\r", [Err(INVALID_LINE_ENDING.clone())]);
    }

    #[test]
    fn single_column_quoted_allow_line_ending_inside() {
        assert_colmatch(CSV, "\"Hello\r\nworld\"", [Ok(~"Hello\r\nworld")]);
    }

    #[test]
    fn single_column_quoted_escaped() {
        assert_colmatch(CSV, r#""Hello, ""rust"" world""#, [Ok(~"Hello, \"rust\" world")]);
        assert_colmatch(Config {escape: Char('$'), ..CSV}, r#""Hello, $"rust$" world""#, [Ok(~"Hello, \"rust\" world")]);
    }

    #[test]
    fn single_column_quoted_escape_char_does_not_end_value() {
        assert_colmatch(Config {escape: Char('~'), ..CSV}, "\"Hello~\r\nworld\"", [Err(IoError {
            kind: io::InvalidInput,
            desc: "Expecting quote char",
            detail: None})]);
    }

    #[test]
    fn single_column_quoted_unexpected_delimiter() {
        assert_colmatch(CSV, r#""ab"c""#, [Err(IoError {
            kind: io::InvalidInput,
            desc: "Expecting line terminator or delimiter",
            detail: None})]);
    }

    #[test]
    fn single_column_quoted_unmatched_quotechar() {
        assert_colmatch(CSV, r#""abc"#, [Err(EOF_ERROR.clone())]);
    }

    #[test]
    fn multi_column_quoted() {
        assert_colmatch(CSV, "\"foo\",\"bar\"", [Ok(~"foo"), Ok(~"bar")]);
        assert_colmatch(QUOTE_TILDE, "~foo~,~bar~", [Ok(~"foo"), Ok(~"bar")]);
        assert_colmatch(Config {delimiter: ';', ..QUOTE_TILDE}, "~foo~;~bar~", [Ok(~"foo"), Ok(~"bar")]);
    }

    #[test]
    fn multi_column_quoted_line_end() {
        assert_colmatch(CSV, "\"foo\",\"bar\"\r\n", [Ok(~"foo"), Ok(~"bar")]);
        assert_colmatch(QUOTE_TILDE, "~foo~,~bar~\r\n", [Ok(~"foo"), Ok(~"bar")]);
        assert_colmatch(Config {delimiter: ';', line_terminator: LF, ..QUOTE_TILDE}, "~foo~;~bar~", [Ok(~"foo"), Ok(~"bar")]);
    }

    #[test]
    fn columns_unquoted_trailing_delim() {
        assert_colmatch(CSV, r#"a,1,c2,"#, [Ok(~"a"), Ok(~"1"), Ok(~"c2"), Ok(~"")]);
        assert_colmatch(DELIM_PIPE, r#"a|1|c2|"#, [Ok(~"a"), Ok(~"1"), Ok(~"c2"), Ok(~"")]);
    }

    #[test]
    fn columns_unquoted_leading_delim() {
        assert_colmatch(CSV, r#",1,c2"#, [Ok(~""), Ok(~"1"), Ok(~"c2")]);
        assert_colmatch(DELIM_PIPE, r#"|1|c2"#, [Ok(~""), Ok(~"1"), Ok(~"c2")]);
    }

    #[test]
    fn columns_quoted_trailing_delim() {
        assert_colmatch(CSV, r#""a","1","c2","#, [Ok(~"a"), Ok(~"1"), Ok(~"c2"), Ok(~"")]);
        assert_colmatch(Config {quote_char: '\'', ..DELIM_PIPE}, r#"'a'|'1'|'c2'|"#, [Ok(~"a"), Ok(~"1"), Ok(~"c2"), Ok(~"")]);
    }

    #[test]
    fn columns_quoted_leading_delim() {
        assert_colmatch(CSV, r#","1","c2""#, [Ok(~""), Ok(~"1"), Ok(~"c2")]);
        assert_colmatch(Config {quote_char: '\'', ..DELIM_PIPE}, r#"|'1'|'c2'"#, [Ok(~""), Ok(~"1"), Ok(~"c2")]);
    }

    #[test]
    fn columns_quoted_escape_before_delimiter_error() {
        assert_colmatch(CSV, r#""foo"","bar""#, [Err(IoError {
            kind: io::InvalidInput,
            desc: "Expecting line terminator or delimiter",
            detail: None})]);
    }

    fn assert_rowmatch(s: &str, ex: Vec<IoResult<Row>>) {
        let reader = io::BufReader::new(s.as_bytes());
        let rows: Vec<IoResult<Row>> = read_rows(CSV, reader).collect();
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
    fn multiple_rows() {
        assert_rowmatch("foo,\"bar\"\r\n\"baz\",qux", vec!(Ok(vec!(~"foo", ~"bar")), Ok(vec!(~"baz", ~"qux"))));
    }

    #[test]
    fn multiple_rows_empty_line_ending() {
        assert_rowmatch("foo,\"bar\"\r\n\"baz\",qux\r\n", vec!(Ok(vec!(~"foo", ~"bar")), Ok(vec!(~"baz", ~"qux"))));
    }

    #[test]
    fn multiple_rows_unclosed_quote() {
        assert_rowmatch("foo,\"bar\r\nbaz,qux", vec!(Err(IoError {kind: io::EndOfFile,
                    desc: "end of file",
                    detail: None})));
    }
}
