// Copyright 2014 Arjan Topolovec
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Library for reading and writing of tabular data.
//!
//! Supported formats:
//!
//! * DSV (Delimiter-separated values):
//!   Most common are CSV (comma-separated values) and TSV (tab-separated values) formats.
//!
//! * Fixed-width columns:
//!   Format where columns are of predefined fixed width, unused width is padded.
//!
//! # Reading is lazy
//!
//! All reading is done on-demand, no reading is done until request for the fist row comes.
//! Rows are read one at a time when requested.
//!
//! # Iteration flexibility
//!
//! Since rows are read as a lazy Iterator you have the flexibility of the Iterator api to control the iteration and row transformation.
//!
//! # Encoder/Decoder api
//!
//! There is no support for Encoder and Decoder api currently (or similar) but library is designed for such extension in the future.
//! Currently best way to achieve similar functionality is mapping a custom decoder over the Rows iterator.
#![crate_id = "tabular"]
#![license = "MIT/ASL2"]
#![crate_type = "lib"]

mod common;

pub mod dsv;
pub mod fixed;
