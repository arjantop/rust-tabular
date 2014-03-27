# Tabular data reader/writer

[![Build Status](https://travis-ci.org/arjantop/rust-tabular.png?branch=master)](https://travis-ci.org/arjantop/rust-tabular)

Reading and writing of tabular data.

- DSV (Delimiter-separated values): CSV (comma-separated values), TSV (tab-separated values)
- Fixed width: formats with columns of fixed width

```
git clone https://github.com/arjantop/rust-tabular
cd rust-tabular
make
```

## Example

Reading CSV data:

```rust
use std::io::BufferedReader;
use std::io::File;

use tabular::dsv::{read_rows, CSV};

let path = Path::new("file.csv");
let mut file = BufferedReader::new(File::open(&path));
for row in read_rows(CSV, file) {
    println!("row = {}", row)
}
```

Reading fixed-length column data:

```rust
use std::io::BufferedReader;
use std::io::File;

use tabular::fixed::{Config, Newline, LF, ColumnConfig, Left, Right, read_rows};

let path = Path::new("file.csv");
let mut file = BufferedReader::new(File::open(&path));

let config = Config {
    columns: vec!(ColumnConfig {width: 5, pad_with: ' ', justification: Left},
                  ColumnConfig {width: 9, pad_with: '-', justification: Right}),
    line_end: Newline(LF)
};

for row in read_rows(config, file) {
    println!("row = {}", row)
}
```

## Documentation

API documentation on [rust-ci.org](http://www.rust-ci.org/arjantop/rust-tabular/doc/tabular/)
