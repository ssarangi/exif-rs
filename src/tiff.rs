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

use crate::endian::{BigEndian, Endian, LittleEndian};
use crate::error::Error;
use crate::ifd::IfdEntry;
use crate::parser::{Parse, Parser};
use crate::tag::{Context, Tag};
use crate::value::get_type_info;
use crate::value::Value;
use crate::{Field, In};

// TIFF header magic numbers [EXIF23 4.5.2].
const TIFF_BE: u16 = 0x4d4d;
const TIFF_LE: u16 = 0x4949;
const TIFF_FORTY_TWO: u16 = 0x002a;
pub const TIFF_BE_SIG: [u8; 4] = [0x4d, 0x4d, 0x00, 0x2a];
pub const TIFF_LE_SIG: [u8; 4] = [0x49, 0x49, 0x2a, 0x00];

/// Parse the Exif attributes in the TIFF format.
///
/// Returns a Vec of Exif fields and a bool.
/// The boolean value is true if the data is little endian.
/// If an error occurred, `exif::Error` is returned.
pub fn parse_exif(data: &[u8]) -> Result<(Vec<Field>, bool), Error> {
    let mut parser = Parser::default();
    parser.parse(data)?;
    let (entries, le) = (parser.entries, parser.little_endian);
    Ok((
        entries
            .into_iter()
            .map(|e| e.into_field(data, le))
            .collect(),
        le,
    ))
}

impl Parse for Parser {
    fn parse(&mut self, data: &[u8]) -> Result<(), Error> {
        // Check the byte order and call the real parser.
        if data.len() < 8 {
            return Err(Error::InvalidFormat("Truncated TIFF header"));
        }
        match BigEndian::loadu16(data, 0) {
            TIFF_BE => {
                self.little_endian = false;
                self.parse_sub::<BigEndian>(data)
            }
            TIFF_LE => {
                self.little_endian = true;
                self.parse_sub::<LittleEndian>(data)
            }
            _ => Err(Error::InvalidFormat("Invalid TIFF byte order")),
        }
    }
}

impl Parser {
    fn parse_sub<E>(&mut self, data: &[u8]) -> Result<(), Error>
    where
        E: Endian,
    {
        // Parse the rest of the header (42 and the IFD offset).
        if E::loadu16(data, 2) != TIFF_FORTY_TWO {
            return Err(Error::InvalidFormat("Invalid forty two"));
        }
        let mut ifd_offset = E::loadu32(data, 4) as usize;
        let mut ifd_num_ck = Some(0);
        while ifd_offset != 0 {
            let ifd_num = ifd_num_ck.ok_or(Error::InvalidFormat("Too many IFDs"))?;
            // Limit the number of IFDs to defend against resource exhaustion
            // attacks.
            if ifd_num >= 8 {
                return Err(Error::InvalidFormat("Limit the IFD count to 8"));
            }
            ifd_offset = self.parse_ifd::<E>(data, ifd_offset, Context::Tiff, ifd_num)?;
            ifd_num_ck = ifd_num.checked_add(1);
        }
        Ok(())
    }

    // Parse IFD [EXIF23 4.6.2].
    fn parse_ifd<E>(
        &mut self,
        data: &[u8],
        offset: usize,
        ctx: Context,
        ifd_num: u16,
    ) -> Result<usize, Error>
    where
        E: Endian,
    {
        // Count (the number of the entries).
        if data.len() < offset || data.len() - offset < 2 {
            return Err(Error::InvalidFormat("Truncated IFD count"));
        }
        let count = E::loadu16(data, offset) as usize;

        // Array of entries.  (count * 12) never overflows.
        if data.len() - offset - 2 < count * 12 {
            return Err(Error::InvalidFormat("Truncated IFD"));
        }
        for i in 0..count as usize {
            let (tag, val) = Self::parse_ifd_entry::<E>(data, offset + 2 + i * 12)?;

            // No infinite recursion will occur because the context is not
            // recursively defined.
            let tag = Tag(ctx, tag);
            match tag {
                Tag::ExifIFDPointer => {
                    self.parse_child_ifd::<E>(data, val, Context::Exif, ifd_num)?
                }
                Tag::GPSInfoIFDPointer => {
                    self.parse_child_ifd::<E>(data, val, Context::Gps, ifd_num)?
                }
                Tag::InteropIFDPointer => {
                    self.parse_child_ifd::<E>(data, val, Context::Interop, ifd_num)?
                }
                _ => self.entries.push(IfdEntry {
                    field: Field {
                        tag: tag,
                        ifd_num: In(ifd_num),
                        value: val,
                    }
                    .into(),
                }),
            }
        }

        // Offset to the next IFD.
        if data.len() - offset - 2 - count * 12 < 4 {
            return Err(Error::InvalidFormat("Truncated next IFD offset"));
        }
        let next_ifd_offset = E::loadu32(data, offset + 2 + count * 12);
        Ok(next_ifd_offset as usize)
    }

    fn parse_ifd_entry<E>(data: &[u8], offset: usize) -> Result<(u16, Value), Error>
    where
        E: Endian,
    {
        // The size of entry has been checked in parse_ifd().
        let tag = E::loadu16(data, offset);
        let typ = E::loadu16(data, offset + 2);
        let cnt = E::loadu32(data, offset + 4);
        let valofs_at = offset + 8;
        let (unitlen, _parser) = get_type_info::<E>(typ);
        let vallen = unitlen
            .checked_mul(cnt as usize)
            .ok_or(Error::InvalidFormat("Invalid entry count"))?;
        let val = if vallen <= 4 {
            Value::Unknown(typ, cnt, valofs_at as u32)
        } else {
            let ofs = E::loadu32(data, valofs_at) as usize;
            if data.len() < ofs || data.len() - ofs < vallen {
                return Err(Error::InvalidFormat("Truncated field value"));
            }
            Value::Unknown(typ, cnt, ofs as u32)
        };
        Ok((tag, val))
    }

    fn parse_child_ifd<E>(
        &mut self,
        data: &[u8],
        mut pointer: Value,
        ctx: Context,
        ifd_num: u16,
    ) -> Result<(), Error>
    where
        E: Endian,
    {
        // The pointer is not yet parsed, so do it here.
        IfdEntry::parse_value::<E>(&mut pointer, data);

        // A pointer field has type == LONG and count == 1, so the
        // value (IFD offset) must be embedded in the "value offset"
        // element of the field.
        let ofs = pointer
            .get_uint(0)
            .ok_or(Error::InvalidFormat("Invalid pointer"))? as usize;
        match self.parse_ifd::<E>(data, ofs, ctx, ifd_num)? {
            0 => Ok(()),
            _ => Err(Error::InvalidFormat("Unexpected next IFD")),
        }
    }
}

pub fn is_tiff(buf: &[u8]) -> bool {
    buf.starts_with(&TIFF_BE_SIG) || buf.starts_with(&TIFF_LE_SIG)
}

#[cfg(test)]
mod tests {
    use crate::ifd::DateTime;

    use super::*;

    #[test]
    fn in_convert() {
        assert_eq!(In::PRIMARY.index(), 0);
        assert_eq!(In::THUMBNAIL.index(), 1);
        assert_eq!(In(2).index(), 2);
        assert_eq!(In(65535).index(), 65535);
        assert_eq!(In::PRIMARY, In(0));
    }

    #[test]
    fn in_display() {
        assert_eq!(format!("{:10}", In::PRIMARY), "primary   ");
        assert_eq!(format!("{:>10}", In::THUMBNAIL), " thumbnail");
        assert_eq!(format!("{:10}", In(2)), "IFD2      ");
        assert_eq!(format!("{:^10}", In(65535)), " IFD65535 ");
    }

    #[test]
    fn truncated() {
        let mut data = b"MM\0\x2a\0\0\0\x08\
              \0\x01\x01\0\0\x03\0\0\0\x01\0\x14\0\0\0\0\0\0"
            .to_vec();
        parse_exif(&data).unwrap();
        while let Some(_) = data.pop() {
            parse_exif(&data).unwrap_err();
        }
    }

    // Before the error is returned, the IFD is parsed multiple times
    // as the 0th, 1st, ..., and n-th IFDs.
    #[test]
    fn inf_loop_by_next() {
        let data = b"MM\0\x2a\0\0\0\x08\
                     \0\x01\x01\0\0\x03\0\0\0\x01\0\x14\0\0\0\0\0\x08";
        assert_err_pat!(
            parse_exif(data),
            Error::InvalidFormat("Limit the IFD count to 8")
        );
    }

    #[test]
    fn inf_loop_by_exif_next() {
        let data = b"MM\x00\x2a\x00\x00\x00\x08\
                     \x00\x01\x87\x69\x00\x04\x00\x00\x00\x01\x00\x00\x00\x1a\
                     \x00\x00\x00\x00\
                     \x00\x01\x90\x00\x00\x07\x00\x00\x00\x040231\
                     \x00\x00\x00\x08";
        assert_err_pat!(
            parse_exif(data),
            Error::InvalidFormat("Unexpected next IFD")
        );
    }

    #[test]
    fn unknown_field() {
        let data = b"MM\0\x2a\0\0\0\x08\
                     \0\x01\x01\0\xff\xff\0\0\0\x01\0\x14\0\0\0\0\0\0";
        let (v, _le) = parse_exif(data).unwrap();
        assert_eq!(v.len(), 1);
        assert_pat!(v[0].value, Value::Unknown(0xffff, 1, 0x12));
    }

    #[test]
    fn parse_ifd_entry() {
        // BYTE (type == 1)
        let data = b"\x02\x03\x00\x01\0\0\0\x04ABCD";
        assert_pat!(
            Parser::parse_ifd_entry::<BigEndian>(data, 0).unwrap(),
            (0x0203, Value::Unknown(1, 4, 8))
        );
        let data = b"\x02\x03\x00\x01\0\0\0\x05\0\0\0\x0cABCDE";
        assert_pat!(
            Parser::parse_ifd_entry::<BigEndian>(data, 0).unwrap(),
            (0x0203, Value::Unknown(1, 5, 12))
        );
        let data = b"\x02\x03\x00\x01\0\0\0\x05\0\0\0\x0cABCD";
        assert_err_pat!(
            Parser::parse_ifd_entry::<BigEndian>(data, 0),
            Error::InvalidFormat("Truncated field value")
        );

        // SHORT (type == 3)
        let data = b"X\x04\x05\x00\x03\0\0\0\x02ABCD";
        assert_pat!(
            Parser::parse_ifd_entry::<BigEndian>(data, 1).unwrap(),
            (0x0405, Value::Unknown(3, 2, 9))
        );
        let data = b"X\x04\x05\x00\x03\0\0\0\x03\0\0\0\x0eXABCDEF";
        assert_pat!(
            Parser::parse_ifd_entry::<BigEndian>(data, 1).unwrap(),
            (0x0405, Value::Unknown(3, 3, 14))
        );
        let data = b"X\x04\x05\x00\x03\0\0\0\x03\0\0\0\x0eXABCDE";
        assert_err_pat!(
            Parser::parse_ifd_entry::<BigEndian>(data, 1),
            Error::InvalidFormat("Truncated field value")
        );

        // Really unknown
        let data = b"X\x01\x02\x03\x04\x05\x06\x07\x08ABCD";
        assert_pat!(
            Parser::parse_ifd_entry::<BigEndian>(data, 1).unwrap(),
            (0x0102, Value::Unknown(0x0304, 0x05060708, 9))
        );
    }

    #[test]
    fn date_time() {
        let mut dt = DateTime::from_ascii(b"2016:05:04 03:02:01").unwrap();
        assert_eq!(dt.year, 2016);
        assert_eq!(dt.to_string(), "2016-05-04 03:02:01");

        dt.parse_subsec(b"987").unwrap();
        assert_eq!(dt.nanosecond.unwrap(), 987000000);
        dt.parse_subsec(b"000987").unwrap();
        assert_eq!(dt.nanosecond.unwrap(), 987000);
        dt.parse_subsec(b"987654321").unwrap();
        assert_eq!(dt.nanosecond.unwrap(), 987654321);
        dt.parse_subsec(b"9876543219").unwrap();
        assert_eq!(dt.nanosecond.unwrap(), 987654321);
        dt.parse_subsec(b"130   ").unwrap();
        assert_eq!(dt.nanosecond.unwrap(), 130000000);
        dt.parse_subsec(b"0").unwrap();
        assert_eq!(dt.nanosecond.unwrap(), 0);
        dt.parse_subsec(b"").unwrap();
        assert!(dt.nanosecond.is_none());
        dt.parse_subsec(b" ").unwrap();
        assert!(dt.nanosecond.is_none());

        dt.parse_offset(b"+00:00").unwrap();
        assert_eq!(dt.offset.unwrap(), 0);
        dt.parse_offset(b"+01:23").unwrap();
        assert_eq!(dt.offset.unwrap(), 83);
        dt.parse_offset(b"+99:99").unwrap();
        assert_eq!(dt.offset.unwrap(), 6039);
        dt.parse_offset(b"-01:23").unwrap();
        assert_eq!(dt.offset.unwrap(), -83);
        dt.parse_offset(b"-99:99").unwrap();
        assert_eq!(dt.offset.unwrap(), -6039);
        assert_err_pat!(dt.parse_offset(b"   :  "), Error::BlankValue(_));
        assert_err_pat!(dt.parse_offset(b"      "), Error::BlankValue(_));
    }

    #[test]
    fn display_value_with_unit() {
        let cm = Field {
            tag: Tag::ResolutionUnit,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![3]),
        };
        let cm_tn = Field {
            tag: Tag::ResolutionUnit,
            ifd_num: In::THUMBNAIL,
            value: Value::Short(vec![3]),
        };
        // No unit.
        let exifver = Field {
            tag: Tag::ExifVersion,
            ifd_num: In::PRIMARY,
            value: Value::Undefined(b"0231".to_vec(), 0),
        };
        assert_eq!(exifver.display_value().to_string(), "2.31");
        assert_eq!(exifver.display_value().with_unit(()).to_string(), "2.31");
        assert_eq!(exifver.display_value().with_unit(&cm).to_string(), "2.31");
        // Fixed string.
        let width = Field {
            tag: Tag::ImageWidth,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![257]),
        };
        assert_eq!(width.display_value().to_string(), "257");
        assert_eq!(
            width.display_value().with_unit(()).to_string(),
            "257 pixels"
        );
        assert_eq!(
            width.display_value().with_unit(&cm).to_string(),
            "257 pixels"
        );
        // Unit tag (with a non-default value).
        // Unit tag is missing but the default is specified.
        let xres = Field {
            tag: Tag::XResolution,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![(300, 1).into()]),
        };
        assert_eq!(xres.display_value().to_string(), "300");
        assert_eq!(
            xres.display_value().with_unit(()).to_string(),
            "300 pixels per inch"
        );
        assert_eq!(
            xres.display_value().with_unit(&cm).to_string(),
            "300 pixels per cm"
        );
        assert_eq!(
            xres.display_value().with_unit(&cm_tn).to_string(),
            "300 pixels per inch"
        );
        // Unit tag is missing and the default is not specified.
        let gpslat = Field {
            tag: Tag::GPSLatitude,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![(10, 1).into(), (0, 1).into(), (1, 10).into()]),
        };
        assert_eq!(gpslat.display_value().to_string(), "10 deg 0 min 0.1 sec");
        assert_eq!(
            gpslat.display_value().with_unit(()).to_string(),
            "10 deg 0 min 0.1 sec [GPSLatitudeRef missing]"
        );
        assert_eq!(
            gpslat.display_value().with_unit(&cm).to_string(),
            "10 deg 0 min 0.1 sec [GPSLatitudeRef missing]"
        );
    }

    #[test]
    fn no_borrow_no_move() {
        let resunit = Field {
            tag: Tag::ResolutionUnit,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![3]),
        };
        // This fails to compile with "temporary value dropped while
        // borrowed" error if with_unit() borrows self.
        let d = resunit.display_value().with_unit(());
        assert_eq!(d.to_string(), "cm");
        // This fails to compile if with_unit() moves self.
        let d1 = resunit.display_value();
        let d2 = d1.with_unit(());
        assert_eq!(d1.to_string(), "cm");
        assert_eq!(d2.to_string(), "cm");
    }
}
