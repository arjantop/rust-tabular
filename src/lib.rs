
// Copyright 2014 Arjan Topolovec
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
  Library for reading and writing of tabular data.

  Supported formats:

  # DSV (Delimiter-separated values):
    Most common are CSV (comma-separated values) and TSV (tab-separated values) formats.

  # Fixed-width columns:
    Format where columns are of predefined fixed width, unused width is padded.
*/
#[crate_id = "tabular"];
#[license = "MIT/ASL2"];
#[crate_type = "rlib"];
#[crate_type = "dylib"];
#[deny(warnings)];

mod common;

pub mod dsv;
pub mod fixed;
