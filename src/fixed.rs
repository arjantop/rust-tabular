//! Reading and writing of data with fixed-width columns and rows
use std::io;
use std::io::{IoResult, IoError};
use std::string::String;

pub use common::{LineTerminator, Row, LF, CR, CRLF, VT, FF, NEL, LS, PS};
use common::INVALID_LINE_ENDING;

/// Text justification
#[deriving(Eq, PartialEq, Clone)]
pub enum Justification {
    /// Justify left, pad right
    Left,
    /// Justify right, pad left
    Right,
}

/// Line ending rule
#[deriving(Eq, PartialEq, Clone)]
pub enum LineEnding {
    /// No row separation, columns of adjacent rows are next to another
    Nothing,
    /// Row is always of set length, unused characters are ignored
    FixedWidth(uint),
    /// Rows are separated by newline line terminator
    Newline(LineTerminator),
}

/// Contains configuration parameters for reading and writing columns
#[deriving(Eq, PartialEq, Clone)]
pub struct ColumnConfig {
    /// Width of column
    pub width: uint,
    /// Character used for padding when data in column < width of column
    pub pad_with: char,
    /// Justification of column data
    pub justification: Justification,
}

/// Contains configuration parameters for reading and writing
#[deriving(Eq, PartialEq, Clone)]
pub struct Config {
    /// Column configurations
    pub columns: Vec<ColumnConfig>,
    /// Line ending rule
    pub line_end: LineEnding,
}

struct Columns<'a, R> {
    reader: &'a mut R,
    config: Config,
    column: uint,
    pos: uint,
    done: bool,
}

impl<'a, R: Buffer> Columns<'a, R> {
    #[inline(always)]
    fn read_char(&mut self) -> IoResult<char> {
        self.pos += 1;
        self.reader.read_char()
    }

    #[inline(always)]
    fn read_str(&mut self, len: uint) -> IoResult<String> {
        let mut s = String::new();
        for _ in range(0, len) {
            match self.read_char() {
                Ok(ch) => s.push_char(ch),
                Err(err) => return Err(err)
            }
        }
        Ok(s.into_string())
    }

    #[inline(always)]
    fn read_column(&mut self, config: ColumnConfig) -> IoResult<String> {
        match self.read_str(config.width) {
            Ok(col) => {
                let trimmed = if config.justification == Left {
                    col.as_slice().trim_right_chars(config.pad_with)
                } else {
                    col.as_slice().trim_left_chars(config.pad_with)
                };
                Ok(trimmed.to_string())
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
        let mut lt = lt.as_str().chars();
        let curr_pos = self.pos;
        for c in lt {
            match self.read_char() {
                Ok(ch) if ch == c => (),
                Ok(_) => return Err(INVALID_LINE_ENDING.clone()),
                Err(ref err) if err.kind == io::EndOfFile && curr_pos + 1 == self.pos => {
                    return Ok(())
                }
                Err(err) => return Err(err)
            }
        }
        Ok(())
    }

    fn read_fixed_width(&mut self, width: uint) -> IoResult<()> {
        try!(self.read_str(width - self.pos));
        Ok(())
    }
}

impl<'a, R: Buffer> Iterator<IoResult<String>> for Columns<'a, R> {
    fn next(&mut self) -> Option<IoResult<String>> {
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

/// Read a single row
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

/// Iterator over rows
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

static COLUMN_TOO_LONG: IoError = IoError {
    kind: io::InvalidInput,
    desc: "Column too long",
    detail: None
};

static ROW_TOO_LONG: IoError = IoError {
    kind: io::InvalidInput,
    desc: "Column too long",
    detail: None
};

/// Create an iterator that reads a line on each iteration until EOF
///
/// ```rust
/// # use std::io::BufferedReader;
/// # use std::io::File;
/// # use tabular::fixed::{Config, Newline, LF, ColumnConfig, Left, Right, read_rows};
/// let path = Path::new("file.csv");
/// let mut file = BufferedReader::new(File::open(&path));
///
/// let config = Config {
///     columns: vec!(ColumnConfig {width: 5, pad_with: ' ', justification: Left},
///                   ColumnConfig {width: 9, pad_with: '-', justification: Right}),
///     line_end: Newline(LF)
/// };
///
/// let rows = read_rows(config, file);
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
/// # use tabular::fixed::{Config, Newline, LF, ColumnConfig, Left, Right, from_str};
/// let config = Config {
///     columns: vec!(ColumnConfig {width: 5, pad_with: ' ', justification: Left},
///                   ColumnConfig {width: 9, pad_with: '-', justification: Right}),
///     line_end: Newline(LF)
/// };
///
/// let rows = from_str(config, "aa,bb\r\ncc,dd");
/// ```
pub fn from_str(config: Config, s: &str) -> RowsMem {
    let buf = io::MemReader::new(Vec::from_slice(s.as_bytes()));
    read_rows(config, buf)
}

pub type RowsFile = Rows<io::BufferedReader<IoResult<io::File>>>;

/// Helper method for reading rows from a file
///
/// ```rust
/// # use tabular::fixed::{Config, Newline, LF, ColumnConfig, Left, Right, from_file};
/// let config = Config {
///     columns: vec!(ColumnConfig {width: 5, pad_with: ' ', justification: Left},
///                   ColumnConfig {width: 9, pad_with: '-', justification: Right}),
///     line_end: Newline(LF)
/// };
///
/// let path = Path::new("path/file.csv");
/// let rows = from_file(config, &path);
/// ```
pub fn from_file(config: Config, path: &Path) -> RowsFile {
    let file = io::BufferedReader::new(io::File::open(path));
    read_rows(config, file)
}


fn write_column(config: &ColumnConfig, writer: &mut Writer, col: &str) -> IoResult<()> {
    if col.len() > config.width {
        return Err(COLUMN_TOO_LONG.clone())
    }
    let padding = config.pad_with.to_str().repeat(config.width - col.len());
    if config.justification == Left {
        try!(writer.write_str(col));
        writer.write_str(padding.as_slice())
    } else {
        try!(writer.write_str(padding.as_slice()));
        writer.write_str(col)
    }
}

/// Write a single row
pub fn write_row(config: &Config, writer: &mut Writer, row: Row) -> IoResult<()> {
    let mut written = 0;
    for (col, cfg) in row.iter().zip(config.columns.iter()) {
        try!(write_column(cfg, writer, col.as_slice()));
        written += cfg.width;
    }
    match config.line_end {
        Nothing => (),
        FixedWidth(w) => {
            if written > w {
                return Err(ROW_TOO_LONG.clone())
            } else {
                let padding = " ".repeat(w - written);
                try!(writer.write_str(padding.as_slice()));
            }
        }
        Newline(lt) => {
            try!(writer.write_str(lt.as_str()));
        }
    }
    Ok(())
}

/// Write rows from iterator into writer with settings from config
///
/// ```rust
/// # #![allow(unused_must_use)]
/// # use std::io::BufferedWriter;
/// # use std::io::File;
/// # use tabular::fixed::{Config, Newline, LF, ColumnConfig, Left, Right, write_rows};
/// let path = Path::new("path/file.csv");
/// let mut file = BufferedWriter::new(File::open(&path));
///
/// let config = Config {
///     columns: vec!(ColumnConfig {width: 5, pad_with: ' ', justification: Left},
///                   ColumnConfig {width: 9, pad_with: '-', justification: Right}),
///     line_end: Newline(LF)
/// };
///
/// let rows = vec!(vec!("a".to_string(), "bb".to_string()), vec!("ccc".to_string(), "dddd".to_string()));
/// write_rows(config, &mut file, rows.move_iter());
/// ```
pub fn write_rows<R: Iterator<Row>>(config: Config, writer: &mut Writer, mut rows: R) -> IoResult<()> {
    for row in rows {
        try!(write_row(&config, writer, row));
    }
    Ok(())
}

/// Helper method for writing rows to a file
///
/// ```rust
/// # #![allow(unused_must_use)]
/// # use tabular::fixed::{Config, Newline, LF, ColumnConfig, Left, Right, write_file};
/// let path = Path::new("path/file.csv");
///
/// let config = Config {
///     columns: vec!(ColumnConfig {width: 5, pad_with: ' ', justification: Left},
///                   ColumnConfig {width: 9, pad_with: '-', justification: Right}),
///     line_end: Newline(LF)
/// };
///
/// let rows = vec!(vec!("a".to_string(), "bb".to_string()), vec!("ccc".to_string(), "dddd".to_string()));
/// write_file(config, &path, rows.move_iter());
/// ```
pub fn write_file<R: Iterator<Row>>(config: Config, path: &Path, rows: R) -> IoResult<()> {
    let mut file = io::BufferedWriter::new(io::File::open_mode(path, io::Open, io::Write));
    write_rows(config, &mut file, rows)
}

#[cfg(test)]
mod test {
    use std::io;
    use std::io::{IoResult, IoError};

    use common::INVALID_LINE_ENDING;

    use super::{Config, ColumnConfig, Left, Right, Row, CRLF, Newline, FixedWidth, LF, Nothing, FF, LS};
    use super::{read_row, read_rows, write_column, COLUMN_TOO_LONG, write_rows, ROW_TOO_LONG, write_row};

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
        assert_colmatch(cfg, "aaa", Ok(vec!("aaa".to_string())));
    }

    #[test]
    fn read_fixed_columns_no_padding() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg, "aaabccccc", Ok(vec!("aaa".to_string(), "b".to_string(), "ccccc".to_string())));
    }

    #[test]
    fn read_fixed_with_zero_length_column() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_ZERO, COLUMN_3),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg, "aaaccccc", Ok(vec!("aaa".to_string(), "".to_string(), "ccccc".to_string())));
    }

    #[test]
    fn read_fixed_columns_with_padding() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Newline(CRLF)
        };
        assert_colmatch(cfg, "  a#cccc-", Ok(vec!("a".to_string(), "".to_string(), "cccc".to_string())));
    }

    #[test]
    fn read_fixed_columns_with_newline_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: Newline(CRLF)
        };
        assert_colmatch(Config {line_end: Newline(LF), ..cfg.clone()}, "aaab\n", Ok(vec!("aaa".to_string(), "b".to_string())));
        assert_colmatch(cfg, "aaab\r\n", Ok(vec!("aaa".to_string(), "b".to_string())));
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
        assert_colmatch(cfg, "aaab      ", Ok(vec!("aaa".to_string(), "b".to_string())));
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
        assert_rowmatch(cfg, " aabccc--\r\n  a#-----", vec!(Ok(vec!("aa".to_string(), "b".to_string(), "ccc".to_string())), Ok(vec!("a".to_string(), "".to_string(), "".to_string()))));
    }

    #[test]
    fn read_lines_with_fixed_columns_and_feedforward_line_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Newline(FF)
        };
        assert_rowmatch(cfg, " aabccc--\x0c  a#-----", vec!(Ok(vec!("aa".to_string(), "b".to_string(), "ccc".to_string())), Ok(vec!("a".to_string(), "".to_string(), "".to_string()))));
    }

    #[test]
    fn read_lines_with_fixed_columns_and_line_separator_line_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Newline(LS)
        };
        assert_rowmatch(cfg, " aabccc--\u2028  a#-----", vec!(Ok(vec!("aa".to_string(), "b".to_string(), "ccc".to_string())), Ok(vec!("a".to_string(), "".to_string(), "".to_string()))));
    }

    #[test]
    fn read_lines_with_fixed_columns_and_fixed_width_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: FixedWidth(10)
        };
        assert_rowmatch(cfg, " aabccc--   a#----- ", vec!(Ok(vec!("aa".to_string(), "b".to_string(), "ccc".to_string())), Ok(vec!("a".to_string(), "".to_string(), "".to_string()))));
    }

    #[test]
    fn read_lines_with_fixed_columns_and_no_line_end() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2, COLUMN_3),
            line_end: Nothing
        };
        assert_rowmatch(cfg, " aabccc--  a#-----", vec!(Ok(vec!("aa".to_string(), "b".to_string(), "ccc".to_string())), Ok(vec!("a".to_string(), "".to_string(), "".to_string()))));
    }

    fn assert_column_written(config: ColumnConfig, col: String, exp: &[u8], exp_res: IoResult<()>) {
        let mut writer = io::MemWriter::new();
        let res = {
            write_column(&config, &mut writer, col.as_slice())
        };
        assert_eq!(res, exp_res);
        assert_eq!(exp, writer.get_ref());
    }

    #[test]
    fn write_zero_width_column() {
        assert_column_written(COLUMN_ZERO, "".to_string(), bytes!(""), Ok(()));
    }

    #[test]
    fn write_fixed_width_column() {
        assert_column_written(COLUMN_1, "aaa".to_string(), bytes!("aaa"), Ok(()));
    }

    #[test]
    fn write_column_with_padding_left() {
        assert_column_written(COLUMN_1, "a".to_string(), bytes!("  a"), Ok(()));
    }

    #[test]
    fn write_column_with_padding_right() {
        assert_column_written(COLUMN_3, "cc".to_string(), bytes!("cc---"), Ok(()));
    }

    #[test]
    fn write_error_on_column_data_too_long() {
        assert_column_written(COLUMN_3, "cccccc".to_string(), bytes!(""), Err(COLUMN_TOO_LONG.clone()));
    }

    #[test]
    fn line_ending_is_written() {
        let config = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: Newline(CRLF)
        };
        let mut writer = io::MemWriter::new();
        let res = {
            let row = vec!("aaa".to_string(), "b".to_string());
            write_row(&config, &mut writer, row)
        };
        assert_eq!(res, Ok(()));
        assert_eq!(writer.get_ref(), bytes!("aaab\r\n"));
    }

    #[test]
    fn write_error_on_fixed_row_columns_too_long() {
        let config = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: FixedWidth(3)
        };
        let mut writer = io::MemWriter::new();
        let res = {
            let row = vec!("aaa".to_string(), "b".to_string());
            write_row(&config, &mut writer, row)
        };
        assert_eq!(res, Err(ROW_TOO_LONG.clone()));
        assert_eq!(writer.get_ref(), bytes!("aaab"));
    }

    fn assert_lines_written(config: Config, rows: Vec<Row>, exp: &[u8], exp_res: IoResult<()>) {
        let mut writer = io::MemWriter::new();
        let res = {
            write_rows(config, &mut writer, rows.move_iter())
        };
        assert_eq!(res, exp_res);
        assert_eq!(writer.get_ref(), exp);
    }

    #[test]
    fn fixed_width_rows_are_written_correctly() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: FixedWidth(6)
        };
        let rows = vec!(vec!("a".to_string(), "".to_string()), vec!("aaa".to_string(), "b".to_string()));
        assert_lines_written(cfg, rows, bytes!("  a#  aaab  "), Ok(()));
    }

    #[test]
    fn newline_terminated_rows_are_written_correctly() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: Newline(LF)
        };
        let rows = vec!(vec!("a".to_string(), "".to_string()), vec!("aaa".to_string(), "b".to_string()));
        assert_lines_written(cfg, rows, bytes!("  a#\naaab\n"), Ok(()));
    }

    #[test]
    fn rows_without_terminator_are_written_correctly() {
        let cfg = Config {
            columns: vec!(COLUMN_1, COLUMN_2),
            line_end: Nothing
        };
        let rows = vec!(vec!("a".to_string(), "".to_string()), vec!("aaa".to_string(), "b".to_string()));
        assert_lines_written(cfg, rows, bytes!("  a#aaab"), Ok(()));
    }
}
