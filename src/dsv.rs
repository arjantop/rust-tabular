//! Reading and writing of DSV (Delimiter-separated values) data
use std::io;
use std::io::{IoResult, IoError};
use std::string::String;

pub use common::{LineTerminator, Row, LF, CR, CRLF, VT, FF, NEL, LS, PS};
use common::INVALID_LINE_ENDING;

/// Quote character inside of quoted column escape rule
#[deriving(Eq, PartialEq, Show)]
pub enum Escape {
    /// Quote character is doubled
    Double,
    /// Quote character is escaped by this character, error if quoted column contains this chosen character
    Char(char),
    /// No escaping is allowed, error is characters that require escaping are in quoted column
    Disallowed,
}

/// Column quoting rule, only Never affects data reading
#[deriving(Eq, PartialEq, Show)]
pub enum Quote {
    /// Column is never quoted, error when writing if it contains characters that should be quoted
    Never,
    /// Column is always quoted
    Always,
    /// Column is quoted if it contains characters that require quoting (delimiter or line terminator)
    Minimal,
}

/// Configuration for RFC 4180 standard CSV parsing
pub static CSV: Config = Config {
    delimiter: ',',
    quote_char: '"',
    escape: Double,
    line_terminator: CRLF,
    quote: Minimal
};

///Configuration for IANA TSV (text/tab-separated-values) parsing
pub static TSV: Config = Config {
    delimiter: '\t',
    quote_char: '\0',
    escape: Disallowed,
    line_terminator: CRLF,
    quote: Never
};

/// Contains configuration parameters for reading and writing
pub struct Config {
    /// Column delimiter
    pub delimiter: char,
    /// Character used for column quoting
    pub quote_char: char,
    /// Quote escape rule
    pub escape: Escape,
    /// Rows are separated by line terminator
    pub line_terminator: LineTerminator,
    /// Quoting of columns
    pub quote: Quote,
}

impl Config {
    fn escape_char(&self) -> Option<char> {
        match self.escape {
            Double => Some(self.quote_char),
            Char(ch) => Some(ch),
            Disallowed => None
        }
    }
}

struct Columns<'a, R: 'a> {
    reader: &'a mut R,
    config: Config,
    row_done: bool,
    done: bool,
    allow_empty: bool,
    column: uint,
    pos: uint
}

impl<'a, R: Buffer> Columns<'a, R> {
    #[inline(always)]
    fn read_char(&mut self) -> IoResult<char> {
        let res = self.reader.read_char();
        if res.is_ok() { self.pos += 1; }
        res
    }

    fn quoted_end(&mut self, next: IoResult<char>, res: String) -> IoResult<String> {
        match next {
            Ok(ch) => {
                if ch == self.config.delimiter {
                    Ok(res)
                } else if self.config.line_terminator.is_beginning(ch) {
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
                self.done = true;
                Ok(res)
            }
            Err(err) => Err(err)
        }
    }

    fn read_quoted_column(&mut self) -> IoResult<String> {
        self.allow_empty = true;
        let mut col = String::new();
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
                        return self.quoted_end(next, col.into_string())
                    } else if ch == self.config.quote_char {
                        let next = self.read_char();
                        match next {
                            Ok(next) if next == self.config.quote_char => {
                                col.push_char(next);
                                continue
                            }
                            _ => ()
                        };
                        return self.quoted_end(next, col.into_string())
                    } else {
                        col.push_char(ch);
                    }
                }
                Err(err) => return Err(err)
            }
        }
    }

    fn read_line_terminator(&mut self) -> IoResult<()> {
        let mut lt = self.config.line_terminator.as_str().chars().skip(1);
        for c in lt {
            match self.read_char() {
                Ok(ch) if ch == c => (),
                Ok(_) => return Err(INVALID_LINE_ENDING.clone()),
                Err(err) => return Err(err)
            }
        }
        self.row_done = true;
        Ok(())
    }

    fn check_eof(&mut self, err: IoError, allow_empty: bool, res: String) -> IoResult<String> {
        if !self.row_done && err.kind == io::EndOfFile && (res.len() > 0 || allow_empty) {
            self.row_done = true;
            self.done = true;
            Ok(res)
        } else {
            Err(err)
        }
    }

    #[inline(always)]
    fn read_unquoted_column(&mut self, mut curr: IoResult<char>) -> IoResult<String> {
        self.allow_empty = false;
        let mut col = String::new();
        loop {
            match curr {
                Ok(ch) => {
                    if self.config.line_terminator.is_beginning(ch) {
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
                Err(err) => {
                    let allow_empty = self.column > 0;
                    return self.check_eof(err, allow_empty, col.into_string())
                }            }
        }
        Ok(col.into_string())
    }

    #[inline(always)]
    fn read_column(&mut self) -> IoResult<String> {
        let res = match self.read_char() {
            Ok(ch) if self.config.quote == Never => self.read_unquoted_column(Ok(ch)),
            Ok(ch) if self.config.quote_char == ch => self.read_quoted_column(),
            res => self.read_unquoted_column(res)
        };
        if res.is_ok() {
            self.column += 1;
        }
        res
    }
}

impl<'a, R: Buffer> Iterator<IoResult<String>> for Columns<'a, R> {
    fn next(&mut self) -> Option<IoResult<String>> {
        if self.row_done {
            return None
        }
        match self.read_column() {
            Err(err) => {
                self.row_done = true;
                if self.pos == 0 && err.kind == io::EndOfFile {
                    self.done = true;
                    None
                } else {
                    Some(Err(err))
                }
            }
            Ok(res) => {
                if self.row_done && !self.allow_empty
                    && self.pos == self.config.line_terminator.as_str().len() {
                    self.next()
                } else {
                    Some(Ok(res))
                }
            }
        }
    }
}

/// Read a single row
pub fn read_row<R: Buffer>(config: Config, reader: &mut R) -> IoResult<Row> {
    let mut res = Vec::new();
    let done = {
        let mut cols = Columns {
            reader: reader,
            config: config,
            row_done: false,
            done: false,
            allow_empty: false,
            column: 0,
            pos: 0
        };
        for col in cols {
            match col {
                Ok(s) => res.push(s),
                Err(err) => return Err(err)
            }
        }
        cols.done
    };
    if res.len() == 0 && !done {
        read_row(config, reader)
    } else {
        Ok(res)
    }
}

///Iterator over rows
pub struct Rows<R> {
    reader: R,
    config: Config,
    done: bool,
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

/// Create an iterator that reads a line on each iteration until EOF
///
/// ```rust
/// # use std::io::BufferedReader;
/// # use std::io::File;
/// # use tabular::dsv::{read_rows, CSV};
/// let path = Path::new("file.csv");
/// let mut file = BufferedReader::new(File::open(&path));
///
/// let rows = read_rows(CSV, file);
/// ```
pub fn read_rows<R: Buffer>(config: Config, reader: R) -> Rows<R> {
    Rows {
        reader: reader,
        config: config,
        done: false
    }
}

pub type RowsMem = Rows<io::MemReader>;

/// Helper method for reading rows from a string
///
/// ```rust
/// # use tabular::dsv::{from_str, CSV};
/// let rows = from_str(CSV, "aa,bb\r\ncc,dd");
/// ```
pub fn from_str(config: Config, s: &str) -> RowsMem {
    let buf = io::MemReader::new(Vec::from_slice(s.as_bytes()));
    read_rows(config, buf)
}

pub type RowsFile = Rows<io::BufferedReader<IoResult<io::File>>>;

/// Helper method for reading rows from a file
///
/// ```rust
/// # use tabular::dsv::{from_file, CSV};
/// let path = Path::new("path/file.csv");
/// let rows = from_file(CSV, &path);
/// ```
pub fn from_file(config: Config, path: &Path) -> RowsFile {
    let file = io::BufferedReader::new(io::File::open(path));
    read_rows(config, file)
}

fn is_quote_required(config: Config, col: &str) -> bool {
    if config.quote == Always {
        return true
    }
    col.chars().any(|ch| {
        ch == config.delimiter || config.line_terminator.is_beginning(ch)
    })
}

static MUST_QUOTE: IoError = IoError {
    kind: io::InvalidInput,
    desc: "Value should be quoted",
    detail: None
};

static ESCAPE_DISALLOWED: IoError = IoError {
    kind: io::InvalidInput,
    desc: "Escaping disallowed",
    detail: None
};

static ESCAPE_CHAR_IN_QUOTE: IoError = IoError {
    kind: io::InvalidInput,
    desc: "Escape characted not allowed in quote",
    detail: None
};

fn write_column(config: Config, writer: &mut Writer, col: &str) -> IoResult<()> {
    if is_quote_required(config, col.as_slice()) {
        if config.quote == Never {
            Err(MUST_QUOTE.clone())
        } else {
            try!(writer.write_char(config.quote_char));
            for ch in col.chars() {
                if ch == config.quote_char {
                    if config.escape_char().is_some() {
                        try!(writer.write_char(config.escape_char().unwrap()));
                    } else {
                        return Err(ESCAPE_DISALLOWED.clone())
                    }
                } else if Some(ch) == config.escape_char() {
                    return Err(ESCAPE_CHAR_IN_QUOTE.clone())
                }
                try!(writer.write_char(ch));
            }
            writer.write_char(config.quote_char)
        }
    } else {
        writer.write(col.as_bytes())
    }
}

/// Write a single row
pub fn write_row(config: Config, writer: &mut Writer, row: Row) -> IoResult<()> {
    let mut first = true;
    for col in row.iter() {
        if !first {
            try!(writer.write_char(config.delimiter));
        }
        try!(write_column(config, writer, col.as_slice()));
        first = false;
    }
    try!(writer.write_str(config.line_terminator.as_str()));
    Ok(())
}

/// Write rows from iterator into writer with settings from config
///
/// ```rust
/// # #![allow(unused_must_use)]
/// # use std::io::BufferedWriter;
/// # use std::io::File;
/// # use tabular::dsv::{write_rows, CSV};
/// let path = Path::new("path/file.csv");
/// let mut file = BufferedWriter::new(File::open(&path));
///
/// let rows = vec!(vec!("a".to_string(), "bb".to_string()), vec!("ccc".to_string(), "dddd".to_string()));
/// write_rows(CSV, &mut file, rows.into_iter());
/// ```
pub fn write_rows<R: Iterator<Row>>(config: Config, writer: &mut Writer, mut rows: R) -> IoResult<()> {
    for row in rows {
        try!(write_row(config, writer, row));
    }
    Ok(())
}

/// Helper method for writing rows to a file
///
/// ```rust
/// # #![allow(unused_must_use)]
/// # use tabular::dsv::{write_file, CSV};
/// let rows = vec!(vec!("a".to_string(), "bb".to_string()), vec!("ccc".to_string(), "dddd".to_string()));
/// let path = Path::new("path/file.csv");
/// write_file(CSV, &path, rows.into_iter());
/// ```
pub fn write_file<R: Iterator<Row>>(config: Config, path: &Path, rows: R) -> IoResult<()> {
    let mut file = io::BufferedWriter::new(io::File::open_mode(path, io::Open, io::Write));
    write_rows(config, &mut file, rows)
}

#[cfg(test)]
mod test {
    use std::io;
    use std::io::{IoResult, IoError};
    use std::vec::Vec;

    use common::INVALID_LINE_ENDING;

    use super::{Columns, Config, Char, CSV, read_rows, Row, LF, TSV, NEL, PS};
    use super::{write_column, write_rows, Never, Always, Disallowed, write_row};
    use super::{ESCAPE_DISALLOWED, MUST_QUOTE, ESCAPE_CHAR_IN_QUOTE};

    fn assert_colmatch(cfg: Config, row: &str, cols: &[IoResult<String>]) {
        let mut reader = io::BufReader::new(row.as_bytes());
        let mut columns = Columns {reader: &mut reader, config: cfg, row_done: false, done: false,
                                    allow_empty: false, column: 0, pos: 0};
        let result: Vec<IoResult<String>> = columns.collect();
        assert_eq!(cols, result.as_slice())
    }

    static DELIM_PIPE: Config = Config {delimiter: '|', ..CSV};

    static QUOTE_TILDE: Config = Config {quote_char: '~', ..CSV};

    static EOF_ERROR: IoError = IoError {
        kind: io::EndOfFile,
        desc: "end of file",
        detail: None
    };

    #[test]
    fn multi_column_quoting_dsabled() {
        assert_colmatch(Config{quote: Never, ..CSV}, "\"foo,bar\"", [Ok("\"foo".to_string()), Ok("bar\"".to_string())]);
    }

    #[test]
    fn empty_column() {
        assert_colmatch(CSV, "", []);
    }

    #[test]
    fn empty_column_line_end() {
        assert_colmatch(CSV, "\r\n", []);
        assert_colmatch(Config {line_terminator: LF, ..CSV}, "\n", []);
    }

    #[test]
    fn single_column() {
        assert_colmatch(CSV, "abc", [Ok("abc".to_string())]);
        assert_colmatch(DELIM_PIPE, "abc", [Ok("abc".to_string())]);
    }

    #[test]
    fn single_column_line_end() {
        assert_colmatch(CSV, "foo\r\n", [Ok("foo".to_string())]);
        assert_colmatch(Config {line_terminator: LF, ..CSV}, "foo\n", [Ok("foo".to_string())]);
    }

    #[test]
    fn single_column_invalid_line_end() {
        assert_colmatch(CSV, "foo\r\r", [Err(INVALID_LINE_ENDING.clone())]);
    }

    #[test]
    fn multi_column() {
        assert_colmatch(CSV, "foo,bar", [Ok("foo".to_string()), Ok("bar".to_string())]);
        assert_colmatch(DELIM_PIPE, "foo|bar", [Ok("foo".to_string()), Ok("bar".to_string())]);
    }

    #[test]
    fn multi_column_line_end() {
        assert_colmatch(CSV, "foo,bar\r\n", [Ok("foo".to_string()), Ok("bar".to_string())]);
        assert_colmatch(DELIM_PIPE, "foo|bar\r\n", [Ok("foo".to_string()), Ok("bar".to_string())]);
        assert_colmatch(Config {line_terminator: LF, ..CSV}, "foo,bar\n", [Ok("foo".to_string()), Ok("bar".to_string())]);
    }

    #[test]
    fn empty_column_quoted() {
        assert_colmatch(CSV, r#""""#, [Ok("".to_string())]);
        assert_colmatch(Config {quote_char: '\'', ..CSV}, "''", [Ok("".to_string())]);
    }

    #[test]
    fn empty_column_quoted_line_end() {
        assert_colmatch(CSV, "\"\"\r\n", [Ok("".to_string())]);
        assert_colmatch(Config {quote_char: '\'', ..CSV}, "''\r\n", [Ok("".to_string())]);
        assert_colmatch(Config {line_terminator: LF, ..CSV}, "\"\"\n", [Ok("".to_string())]);
        assert_colmatch(Config {line_terminator: LF, quote_char: '\'', ..CSV}, "''\n", [Ok("".to_string())]);
    }

    #[test]
    fn single_column_quoted() {
        assert_colmatch(CSV, r#""abc""#, [Ok("abc".to_string())]);
        assert_colmatch(QUOTE_TILDE, r#"~abc~"#, [Ok("abc".to_string())]);
    }

    #[test]
    fn single_column_quoted_with_delim() {
        assert_colmatch(CSV, r#""a,b,c""#, [Ok("a,b,c".to_string())]);
        assert_colmatch(Config {delimiter: '-', ..QUOTE_TILDE}, r#"~a-b-c~"#, [Ok("a-b-c".to_string())]);
    }

    #[test]
    fn single_column_quoted_line_end() {
        assert_colmatch(CSV, "\"abc\"\r\n", [Ok("abc".to_string())]);
        assert_colmatch(QUOTE_TILDE, "~abc~\r\n", [Ok("abc".to_string())]);
        assert_colmatch(Config {line_terminator: LF, ..QUOTE_TILDE}, "~abc~\n", [Ok("abc".to_string())]);
    }

    #[test]
    fn single_column_quoted_invalid_line_end() {
        assert_colmatch(CSV, "\"abc\"\r\r", [Err(INVALID_LINE_ENDING.clone())]);
    }

    #[test]
    fn single_column_quoted_allow_line_ending_inside() {
        assert_colmatch(CSV, "\"Hello\r\nworld\"", [Ok("Hello\r\nworld".to_string())]);
    }

    #[test]
    fn single_column_quoted_escaped() {
        assert_colmatch(CSV, r#""Hello, ""rust"" world""#, [Ok("Hello, \"rust\" world".to_string())]);
        assert_colmatch(Config {escape: Char('$'), ..CSV}, r#""Hello, $"rust$" world""#, [Ok("Hello, \"rust\" world".to_string())]);
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
        assert_colmatch(CSV, "\"foo\",\"bar\"", [Ok("foo".to_string()), Ok("bar".to_string())]);
        assert_colmatch(QUOTE_TILDE, "~foo~,~bar~", [Ok("foo".to_string()), Ok("bar".to_string())]);
        assert_colmatch(Config {delimiter: ';', ..QUOTE_TILDE}, "~foo~;~bar~", [Ok("foo".to_string()), Ok("bar".to_string())]);
    }

    #[test]
    fn multi_column_quoted_line_end() {
        assert_colmatch(CSV, "\"foo\",\"bar\"\r\n", [Ok("foo".to_string()), Ok("bar".to_string())]);
        assert_colmatch(QUOTE_TILDE, "~foo~,~bar~\r\n", [Ok("foo".to_string()), Ok("bar".to_string())]);
        assert_colmatch(Config {delimiter: ';', line_terminator: LF, ..QUOTE_TILDE}, "~foo~;~bar~", [Ok("foo".to_string()), Ok("bar".to_string())]);
    }

    #[test]
    fn columns_unquoted_trailing_delim() {
        assert_colmatch(CSV, r#"a,1,c2,"#, [Ok("a".to_string()), Ok("1".to_string()), Ok("c2".to_string()), Ok("".to_string())]);
        assert_colmatch(DELIM_PIPE, r#"a|1|c2|"#, [Ok("a".to_string()), Ok("1".to_string()), Ok("c2".to_string()), Ok("".to_string())]);
    }

    #[test]
    fn columns_unquoted_leading_delim() {
        assert_colmatch(CSV, r#",1,c2"#, [Ok("".to_string()), Ok("1".to_string()), Ok("c2".to_string())]);
        assert_colmatch(DELIM_PIPE, r#"|1|c2"#, [Ok("".to_string()), Ok("1".to_string()), Ok("c2".to_string())]);
    }

    #[test]
    fn columns_quoted_trailing_delim() {
        assert_colmatch(CSV, r#""a","1","c2","#, [Ok("a".to_string()), Ok("1".to_string()), Ok("c2".to_string()), Ok("".to_string())]);
        assert_colmatch(Config {quote_char: '\'', ..DELIM_PIPE}, r#"'a'|'1'|'c2'|"#, [Ok("a".to_string()), Ok("1".to_string()), Ok("c2".to_string()), Ok("".to_string())]);
    }

    #[test]
    fn columns_quoted_leading_delim() {
        assert_colmatch(CSV, r#","1","c2""#, [Ok("".to_string()), Ok("1".to_string()), Ok("c2".to_string())]);
        assert_colmatch(Config {quote_char: '\'', ..DELIM_PIPE}, r#"|'1'|'c2'"#, [Ok("".to_string()), Ok("1".to_string()), Ok("c2".to_string())]);
    }

    #[test]
    fn columns_quoted_escape_before_delimiter_error() {
        assert_colmatch(CSV, r#""foo"","bar""#, [Err(IoError {
            kind: io::InvalidInput,
            desc: "Expecting line terminator or delimiter",
            detail: None})]);
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
    fn multiple_rows() {
        assert_rowmatch(CSV, "foo,\"bar\"\r\n\"baz\",qux", vec!(Ok(vec!("foo".to_string(), "bar".to_string())), Ok(vec!("baz".to_string(), "qux".to_string()))));
    }

    #[test]
    fn empty_lines_are_ignored() {
        assert_rowmatch(CSV, "aa,bb\r\n\r\n\r\ncc,dd", vec!(Ok(vec!("aa".to_string(), "bb".to_string())), Ok(vec!("cc".to_string(), "dd".to_string()))));
    }


    #[test]
    fn multiple_rows_empty_line_ending() {
        assert_rowmatch(CSV, "foo,\"bar\"\r\n\"baz\",qux\r\n", vec!(Ok(vec!("foo".to_string(), "bar".to_string())), Ok(vec!("baz".to_string(), "qux".to_string()))));
    }

    #[test]
    fn read_tsv() {
        assert_rowmatch(TSV, "foo\tbar\r\nbaz\tqux", vec!(Ok(vec!("foo".to_string(), "bar".to_string())), Ok(vec!("baz".to_string(), "qux".to_string()))));
    }

    #[test]
    fn read_nextline_line_terminated() {
        assert_rowmatch(Config {line_terminator: NEL, ..CSV}, "foo,bar\x85baz,qux", vec!(Ok(vec!("foo".to_string(), "bar".to_string())), Ok(vec!("baz".to_string(), "qux".to_string()))));
    }

    #[test]
    fn read_paragraph_separator_line_terminated() {
        assert_rowmatch(Config {line_terminator: PS, ..CSV}, "foo,bar\u2029baz,qux", vec!(Ok(vec!("foo".to_string(), "bar".to_string())), Ok(vec!("baz".to_string(), "qux".to_string()))));
    }

    #[test]
    fn multiple_rows_unclosed_quote() {
        assert_rowmatch(CSV, "foo,\"bar\r\nbaz,qux", vec!(Err(IoError {kind: io::EndOfFile,
                    desc: "end of file",
                    detail: None})));
    }

    fn assert_column_written(config: Config, col: String, exp: &[u8], exp_res: IoResult<()>) {
        let mut writer = io::MemWriter::new();
        let res = {
            write_column(config, &mut writer, col.as_slice())
        };
        assert_eq!(res, exp_res);
        assert_eq!(exp, writer.get_ref());
    }

    #[test]
    fn written_column_is_not_quoted() {
        assert_column_written(CSV, "foo".to_string(), b"foo", Ok(()));
        assert_column_written(Config {quote: Never, ..CSV}, "foo".to_string(), b"foo", Ok(()));
    }

    #[test]
    fn written_column_is_quoted() {
        assert_column_written(CSV, "fo,o".to_string(), b"\"fo,o\"", Ok(()));
        assert_column_written(CSV, "f\ro".to_string(), b"\"f\ro\"", Ok(()));
        assert_column_written(Config {quote: Always, ..CSV}, "bar".to_string(), b"\"bar\"", Ok(()));
    }

    #[test]
    fn error_on_writing_value_that_should_be_quoted() {
        assert_column_written(Config {quote: Never, ..DELIM_PIPE}, "a|b".to_string(), b"", Err(MUST_QUOTE.clone()))
    }

    #[test]
    fn written_column_containing_quote_char_is_quoted() {
        assert_column_written(CSV, "Hello, \"world\"".to_string(), b"\"Hello, \"\"world\"\"\"", Ok(()));
        assert_column_written(Config {escape: Char('!'), ..QUOTE_TILDE}, "Hello, ~world~".to_string(), b"~Hello, !~world!~~", Ok(()));
    }

    #[test]
    fn error_when_writing_quoted_column_with_escape_disallowed() {
        assert_column_written(Config {escape: Disallowed, ..QUOTE_TILDE}, "Hello, ~world~".to_string(), b"~Hello, ", Err(ESCAPE_DISALLOWED.clone()));
    }

    #[test]
    fn writen_quoted_column_can_not_cantain_escape_char() {
        assert_column_written(Config {escape: Char('?'), quote: Always, ..CSV}, "Hello?".to_string(), b"\"Hello", Err(ESCAPE_CHAR_IN_QUOTE.clone()));
    }

    #[test]
    fn line_ending_is_written() {
        let mut writer = io::MemWriter::new();
        let res = {
            let rows = vec!("foo".to_string(), "bar".to_string());
            write_row(CSV, &mut writer, rows)
        };
        assert_eq!(Ok(()), res);
        assert_eq!(b"foo,bar\r\n", writer.get_ref());
    }

    #[test]
    fn rows_are_written_correctly() {
        let mut writer = io::MemWriter::new();
        let res = {
            let rows = vec!(vec!("foo".to_string(), "b|ar".to_string()), vec!("b\r\naz".to_string(), "qux".to_string()));
            write_rows(DELIM_PIPE, &mut writer, rows.into_iter())
        };
        assert_eq!(Ok(()), res);
        assert_eq!(b"foo|\"b|ar\"\r\n\"b\r\naz\"|qux\r\n", writer.get_ref());
    }
}

#[cfg(test)]
mod bench {
    extern crate test;

    use self::test::Bencher;

    use super::{from_file, CSV};

    #[bench]
    fn read_medium(b: &mut Bencher) {
        let path = Path::new("data/medium.csv");
        b.iter(|| {
            for _ in from_file(CSV, &path) {}
        })
    }

    #[bench]
    fn read_short(b: &mut Bencher) {
        let path = Path::new("data/short.csv");
        b.iter(|| {
            for _ in from_file(CSV, &path) {}
        })
    }
}
