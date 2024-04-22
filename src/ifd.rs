use std::fmt;

use mutate_once::MutOnce;

use crate::{
    endian::{BigEndian, Endian, LittleEndian},
    tag::UnitPiece,
    util::{atou16, ctou32},
    value::{self, get_type_info},
    Error, Tag, Value,
};

// Partially parsed TIFF field (IFD entry).
// Value::Unknown is abused to represent a partially parsed value.
// Such a value must never be exposed to the users of this library.
#[derive(Debug)]
pub struct IfdEntry {
    // When partially parsed, the value is stored as Value::Unknown.
    // Do not leak this field to the outside.
    pub field: MutOnce<Field>,
}

impl IfdEntry {
    pub fn ifd_num_tag(&self) -> (In, Tag) {
        if self.field.is_fixed() {
            let field = self.field.get_ref();
            (field.ifd_num, field.tag)
        } else {
            let field = self.field.get_mut();
            (field.ifd_num, field.tag)
        }
    }

    pub fn ref_field<'a>(&'a self, data: &[u8], le: bool) -> &'a Field {
        self.parse(data, le);
        self.field.get_ref()
    }

    pub fn into_field(self, data: &[u8], le: bool) -> Field {
        self.parse(data, le);
        self.field.into_inner()
    }

    fn parse(&self, data: &[u8], le: bool) {
        if !self.field.is_fixed() {
            let mut field = self.field.get_mut();
            if le {
                Self::parse_value::<LittleEndian>(&mut field.value, data);
            } else {
                Self::parse_value::<BigEndian>(&mut field.value, data);
            }
        }
    }

    // Converts a partially parsed value into a real one.
    pub fn parse_value<E>(value: &mut Value, data: &[u8])
    where
        E: Endian,
    {
        match *value {
            Value::Unknown(typ, cnt, ofs) => {
                let (unitlen, parser) = get_type_info::<E>(typ);
                if unitlen != 0 {
                    *value = parser(data, ofs as usize, cnt as usize);
                }
            }
            _ => {
                // Do nothing... everything is successfully parsed
            }
        }
    }
}

/// A TIFF/Exif field.
#[derive(Debug, Clone)]
pub struct Field {
    /// The tag of this field.
    pub tag: Tag,
    /// The index of the IFD to which this field belongs.
    pub ifd_num: In,
    /// The value of this field.
    pub value: Value,
}

/// An IFD number.
///
/// The IFDs are indexed from 0.  The 0th IFD is for the primary image
/// and the 1st one is for the thumbnail.  Two associated constants,
/// `In::PRIMARY` and `In::THUMBNAIL`, are defined for them respectively.
///
/// # Examples
/// ```
/// use exif::In;
/// assert_eq!(In::PRIMARY.index(), 0);
/// assert_eq!(In::THUMBNAIL.index(), 1);
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct In(pub u16);

impl In {
    pub const PRIMARY: In = In(0);
    pub const THUMBNAIL: In = In(1);

    /// Returns the IFD number.
    #[inline]
    pub fn index(self) -> u16 {
        self.0
    }
}

impl fmt::Display for In {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            0 => f.pad("primary"),
            1 => f.pad("thumbnail"),
            n => f.pad(&format!("IFD{}", n)),
        }
    }
}

/// A struct used to parse a DateTime field.
///
/// # Examples
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use exif::DateTime;
/// let dt = DateTime::from_ascii(b"2016:05:04 03:02:01")?;
/// assert_eq!(dt.year, 2016);
/// assert_eq!(dt.to_string(), "2016-05-04 03:02:01");
/// # Ok(()) }
/// ```
#[derive(Debug)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    /// The subsecond data in nanoseconds.  If the Exif attribute has
    /// more sigfinicant digits, they are rounded down.
    pub nanosecond: Option<u32>,
    /// The offset of the time zone in minutes.
    pub offset: Option<i16>,
}

impl DateTime {
    /// Parse an ASCII data of a DateTime field.  The range of a number
    /// is not validated, so, for example, 13 may be returned as the month.
    ///
    /// If the value is blank, `Error::BlankValue` is returned.
    pub fn from_ascii(data: &[u8]) -> Result<DateTime, Error> {
        if data == b"    :  :     :  :  " || data == b"                   " {
            return Err(Error::BlankValue("DateTime is blank"));
        } else if data.len() < 19 {
            return Err(Error::InvalidFormat("DateTime too short"));
        } else if !(data[4] == b':'
            && data[7] == b':'
            && data[10] == b' '
            && data[13] == b':'
            && data[16] == b':')
        {
            return Err(Error::InvalidFormat("Invalid DateTime delimiter"));
        }
        Ok(DateTime {
            year: atou16(&data[0..4])?,
            month: atou16(&data[5..7])? as u8,
            day: atou16(&data[8..10])? as u8,
            hour: atou16(&data[11..13])? as u8,
            minute: atou16(&data[14..16])? as u8,
            second: atou16(&data[17..19])? as u8,
            nanosecond: None,
            offset: None,
        })
    }

    /// Parses an SubsecTime-like field.
    pub fn parse_subsec(&mut self, data: &[u8]) -> Result<(), Error> {
        let mut subsec = 0;
        let mut ndigits = 0;
        for &c in data {
            if c == b' ' {
                break;
            }
            subsec = subsec * 10 + ctou32(c)?;
            ndigits += 1;
            if ndigits >= 9 {
                break;
            }
        }
        if ndigits == 0 {
            self.nanosecond = None;
        } else {
            for _ in ndigits..9 {
                subsec *= 10;
            }
            self.nanosecond = Some(subsec);
        }
        Ok(())
    }

    /// Parses an OffsetTime-like field.
    pub fn parse_offset(&mut self, data: &[u8]) -> Result<(), Error> {
        if data == b"   :  " || data == b"      " {
            return Err(Error::BlankValue("OffsetTime is blank"));
        } else if data.len() < 6 {
            return Err(Error::InvalidFormat("OffsetTime too short"));
        } else if data[3] != b':' {
            return Err(Error::InvalidFormat("Invalid OffsetTime delimiter"));
        }
        let hour = atou16(&data[1..3])?;
        let min = atou16(&data[4..6])?;
        let offset = (hour * 60 + min) as i16;
        self.offset = Some(match data[0] {
            b'+' => offset,
            b'-' => -offset,
            _ => return Err(Error::InvalidFormat("Invalid OffsetTime sign")),
        });
        Ok(())
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

impl Field {
    /// Returns an object that implements `std::fmt::Display` for
    /// printing the value of this field in a tag-specific format.
    ///
    /// To print the value with the unit, call `with_unit` method on the
    /// returned object.  It takes a parameter, which is either `()`,
    /// `&Field`, or `&Exif`, that provides the unit information.
    /// If the unit does not depend on another field, `()` can be used.
    /// Otherwise, `&Field` or `&Exif` should be used.
    ///
    /// # Examples
    ///
    /// ```
    /// use exif::{Field, In, Tag, Value};
    ///
    /// let xres = Field {
    ///     tag: Tag::XResolution,
    ///     ifd_num: In::PRIMARY,
    ///     value: Value::Rational(vec![(72, 1).into()]),
    /// };
    /// let resunit = Field {
    ///     tag: Tag::ResolutionUnit,
    ///     ifd_num: In::PRIMARY,
    ///     value: Value::Short(vec![3]),
    /// };
    /// assert_eq!(xres.display_value().to_string(), "72");
    /// assert_eq!(resunit.display_value().to_string(), "cm");
    /// // The unit of XResolution is indicated by ResolutionUnit.
    /// assert_eq!(xres.display_value().with_unit(&resunit).to_string(),
    ///            "72 pixels per cm");
    /// // If ResolutionUnit is not given, the default value is used.
    /// assert_eq!(xres.display_value().with_unit(()).to_string(),
    ///            "72 pixels per inch");
    /// assert_eq!(xres.display_value().with_unit(&xres).to_string(),
    ///            "72 pixels per inch");
    ///
    /// let flen = Field {
    ///     tag: Tag::FocalLengthIn35mmFilm,
    ///     ifd_num: In::PRIMARY,
    ///     value: Value::Short(vec![24]),
    /// };
    /// // The unit of the focal length is always mm, so the argument
    /// // has nothing to do with the result.
    /// assert_eq!(flen.display_value().with_unit(()).to_string(),
    ///            "24 mm");
    /// assert_eq!(flen.display_value().with_unit(&resunit).to_string(),
    ///            "24 mm");
    /// ```
    #[inline]
    pub fn display_value(&self) -> DisplayValue {
        DisplayValue {
            tag: self.tag,
            ifd_num: self.ifd_num,
            value_display: self.value.display_as(self.tag),
        }
    }
}

/// Helper struct for printing a value in a tag-specific format.
pub struct DisplayValue<'a> {
    tag: Tag,
    ifd_num: In,
    value_display: value::Display<'a>,
}

impl<'a> DisplayValue<'a> {
    #[inline]
    pub fn with_unit<T>(&self, unit_provider: T) -> DisplayValueUnit<'a, T>
    where
        T: ProvideUnit<'a>,
    {
        DisplayValueUnit {
            ifd_num: self.ifd_num,
            value_display: self.value_display,
            unit: self.tag.unit(),
            unit_provider: unit_provider,
        }
    }
}

impl<'a> fmt::Display for DisplayValue<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.value_display.fmt(f)
    }
}

/// Helper struct for printing a value with its unit.
pub struct DisplayValueUnit<'a, T>
where
    T: ProvideUnit<'a>,
{
    ifd_num: In,
    value_display: value::Display<'a>,
    unit: Option<&'static [UnitPiece]>,
    unit_provider: T,
}

impl<'a, T> fmt::Display for DisplayValueUnit<'a, T>
where
    T: ProvideUnit<'a>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(unit) = self.unit {
            assert!(!unit.is_empty());
            for piece in unit {
                match *piece {
                    UnitPiece::Value => self.value_display.fmt(f),
                    UnitPiece::Str(s) => f.write_str(s),
                    UnitPiece::Tag(tag) => {
                        if let Some(x) = self.unit_provider.get_field(tag, self.ifd_num) {
                            x.value.display_as(tag).fmt(f)
                        } else if let Some(x) = tag.default_value() {
                            x.display_as(tag).fmt(f)
                        } else {
                            write!(f, "[{} missing]", tag)
                        }
                    }
                }?
            }
            Ok(())
        } else {
            self.value_display.fmt(f)
        }
    }
}

pub trait ProvideUnit<'a>: Copy {
    fn get_field(self, tag: Tag, ifd_num: In) -> Option<&'a Field>;
}

impl<'a> ProvideUnit<'a> for () {
    fn get_field(self, _tag: Tag, _ifd_num: In) -> Option<&'a Field> {
        None
    }
}

impl<'a> ProvideUnit<'a> for &'a Field {
    fn get_field(self, tag: Tag, ifd_num: In) -> Option<&'a Field> {
        Some(self).filter(|x| x.tag == tag && x.ifd_num == ifd_num)
    }
}
