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

extern crate exif;

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

fn main() {
    for path in env::args_os().skip(1).map(PathBuf::from) {
        if let Err(e) = dump_file(&path) {
            println!("{}: {}", path.display(), e);
        }
    }
}

fn dump_file(path: &Path) -> Result<(), exif::Error> {
    let file = File::open(path)?;
    let exif = exif::Reader::new().read_from_container(&mut BufReader::new(&file))?;

    println!("{}", path.display());
    for f in exif.fields() {
        println!(
            "  {}/{} --> : {}",
            f.ifd_num.index(),
            f.tag,
            f.display_value().with_unit(&exif)
        );
        println!("      {:?}", f.value);
    }

    // Dump the LensMake tag of the first IFD.
    if let Some(f) = exif.get_field(exif::Tag::LensMake, exif::In::PRIMARY) {
        println!("LensMake: {}", f.display_value().with_unit(&exif));
    }

    // Dump the LensModel tag of the first IFD.
    if let Some(f) = exif.get_field(exif::Tag::LensModel, exif::In::PRIMARY) {
        println!("LensModel: {}", f.display_value().with_unit(&exif));
    }
    Ok(())
}
