use std::collections::HashMap;

use crate::{
    ifd::{IfdEntry, ProvideUnit},
    Field, In, Tag,
};

/// A struct that holds the parsed Exif attributes.
///
/// # Examples
/// ```
/// # fn main() { sub(); }
/// # fn sub() -> Option<()> {
/// # use exif::{In, Reader, Tag};
/// # let file = std::fs::File::open("tests/exif.jpg").unwrap();
/// # let exif = Reader::new().read_from_container(
/// #     &mut std::io::BufReader::new(&file)).unwrap();
/// // Get a specific field.
/// let xres = exif.get_field(Tag::XResolution, In::PRIMARY)?;
/// assert_eq!(xres.display_value().with_unit(&exif).to_string(),
///            "72 pixels per inch");
/// // Iterate over all fields.
/// for f in exif.fields() {
///     println!("{} {} {}", f.tag, f.ifd_num, f.display_value());
/// }
/// # Some(()) }
/// ```
#[derive(Debug)]
pub struct Exif {
    // TIFF data.
    pub buf: Vec<u8>,
    // Exif fields.  Vec is used to keep the ability to enumerate all fields
    // even if there are duplicates.
    pub entries: Vec<IfdEntry>,
    // HashMap to the index of the Vec for faster random access.
    pub entry_map: HashMap<(In, Tag), usize>,
    // True if the TIFF data is little endian.
    pub little_endian: bool,
    // pub exif: Box<Exif>,
}

impl Exif {
    /// Returns the slice that contains the TIFF data.
    #[inline]
    pub fn buf(&self) -> &[u8] {
        &self.buf[..]
    }

    /// Returns an iterator of Exif fields.
    #[inline]
    pub fn fields(&self) -> impl ExactSizeIterator<Item = &Field> {
        self.entries
            .iter()
            .map(move |e| e.ref_field(&self.buf, self.little_endian))
    }

    /// Returns true if the Exif data (TIFF structure) is in the
    /// little-endian byte order.
    #[inline]
    pub fn little_endian(&self) -> bool {
        self.little_endian
    }

    /// Returns a reference to the Exif field specified by the tag
    /// and the IFD number.
    #[inline]
    pub fn get_field(&self, tag: Tag, ifd_num: In) -> Option<&Field> {
        self.entry_map
            .get(&(ifd_num, tag))
            .map(|&i| self.entries[i].ref_field(&self.buf, self.little_endian))
    }

    #[inline]
    pub fn merge_two_exif(exif1: &Exif, exif2: &Exif) -> Exif {
        let mut new_entries = exif1.entries.clone();
        let mut new_entry_map = exif1.entry_map.clone();

        let offset = exif1.entries.len();

        // Append exif2 entries and adjust entry_map
        new_entries.extend(exif2.entries.iter().cloned());
        for (key, &index) in exif2.entry_map.iter() {
            new_entry_map.insert(*key, index + offset);
        }

        Exif {
            buf: vec![], // Not merging buffers due to complexity
            entries: new_entries,
            entry_map: new_entry_map,
            little_endian: exif1.little_endian, // Assumes both have the same endianness
        }
    }
}

impl<'a> ProvideUnit<'a> for &'a Exif {
    fn get_field(self, tag: Tag, ifd_num: In) -> Option<&'a Field> {
        self.get_field(tag, ifd_num)
    }
}
