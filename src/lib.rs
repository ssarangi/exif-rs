//
// Copyright (c) 2016 KAMADA Ken'ichi.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
// 1. Redistributions of source code must retain the above copyright
//    notice, this list of conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright
//    notice, this list of conditions and the following disclaimer in the
//    documentation and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
// OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
// HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
// LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
// OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
// SUCH DAMAGE.
//

//! This is a pure-Rust library to parse Exif data.
//! This library can parse TIFF and JPEG images and extract Exif
//! attributes.
//!
//! # Examples
//!
//! An example to parse JPEG/TIFF files:
//!
//! ```
//! for path in &["tests/exif.jpg", "tests/exif.tif"] {
//!     let file = std::fs::File::open(path).unwrap();
//!     let reader = exif::Reader::new(
//!         &mut std::io::BufReader::new(&file)).unwrap();
//!     for f in reader.fields() {
//!         println!("{} {} {}",
//!                  f.tag, f.ifd_num, f.display_value().with_unit(&reader));
//!     }
//! }
//! ```
//!
//! # Compatibility
//!
//! Major changes between 0.3.1 and 0.4 are listed below.
//!
//! * The constants in tag module (`tag::TagName`) have been removed.
//!   Use `Tag::TagName` instead.
//! * Sturct `In` (IFD number) has been added to indicate primary/thumbnail
//!   images, which were distinguished by `bool` previously.  Function
//!   parameters and struct members now take `In`s instead of `bool`s.
//!   `Field::thumbnail` was renamed to `Field::ifd_num` accordingly.
//! * The type of `Context` was changed from enum to struct.  The variants
//!   (e.g., `Context::Tiff`) were changed to associated constants and
//!   they are now spelled in all uppercase (e.g., `Context::TIFF`).
//! * `Value` became a self-contained type.  The structures of `Value::Ascii`
//!   and `Value::Undefined` have been changed to use Vec<u8> instead of &[u8].

pub use error::Error;
pub use jpeg::get_exif_attr as get_exif_attr_from_jpeg;
pub use reader::Reader;
pub use tag::{Context, Tag};
pub use tiff::{DateTime, Field, In};
pub use tiff::parse_exif_compat03 as parse_exif;
pub use value::Value;
pub use value::{Rational, SRational};

/// The interfaces in this module are experimental and unstable.
pub mod experimental {
    pub use crate::writer::Writer;
}

#[cfg(test)]
#[macro_use]
mod tmacro;

mod endian;
mod error;
mod jpeg;
mod reader;
mod tag;
mod tiff;
mod util;
mod value;
mod writer;
