use std::io::{BufRead, Read};
use std::mem::size_of;

use crate::error::Error;
use crate::ifd::IfdEntry;
use crate::parser::{self, Parse};
use crate::util::{read16, read32};
use crate::{jpeg, Context, Exif, Field, In, Tag, Value};
use byteorder::ReadBytesExt;

use std::collections::HashMap;
use std::io::SeekFrom;

use std::io::{self, Seek};

use num_derive::FromPrimitive;

/*
Based on the following: http://fileformats.archiveteam.org/wiki/Fujifilm_RAF
* Format
** Byte order is Motorola (Big Endian)

*** 16 bytes string to identify the file (magic)
**** "FUJIFILMCCD-RAW "
*** 4 bytes
**** Format version. E.g. "0201"
*** 8 bytes
**** Camera number ID. E.g. "FF389501"
*** 32 bytes for the camera string, \0 terminated
*** offset directory
**** Version (4 bytes) for the directory. E.g. "0100" or "0159".
**** 20 bytes "unknown"
**** Jpeg image offset (4 bytes)
**** Jpeg Image length (4 bytes)
**** CFA Header Offset (4 bytes)
**** CFA Header Length (4 bytes)
**** CFA Offset (4 bytes)
**** CFA Length (4 bytes)
**** rest unused
*** Jpeg image offset
**** Exif JFIF with thumbnail + preview
*** CFA Header offset - Big Endian
**** 4 bytes: count of records
**** Records, one after the other
***** 2 bytes: tag ID
***** 2 bytes: size of record (N)
***** N bytes: data
*** CFA Offset
**** Uncompressed RAW
*/

mod marker {
    // The first byte of a marker
    pub const TIFF1_JPEG_PTR_OFFSET: usize = 84;
    pub const TIFF2_PTR_OFFSET: usize = 100;
    pub const TAGS_PTR_OFFSET: usize = 92;
}

/// Specific RAF Makernotes tags.
/// These are only related to the Makernote IFD.
#[derive(Debug, Copy, Clone, PartialEq, enumn::N, FromPrimitive)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum RafMakernotes {
    Version = 0x0000,
    InternalSerialNumber = 0x0010,
    Quality = 0x1000,
    Sharpness = 0x1001,
    WhiteBalance = 0x1002,
    Saturation = 0x1003,
    Contrast = 0x1004,
    ColorTemperature = 0x1005,
    Contrast2 = 0x1006,
    WhiteBalanceFineTune = 0x100a,
    NoiseReduction = 0x100b,
    NoiseReduction2 = 0x100e,
    FujiFlashMode = 0x1010,
    FlashExposureComp = 0x1011,
    Macro = 0x1020,
    FocusMode = 0x1021,
    AFMode = 0x1022,
    FocusPixel = 0x1023,
    PrioritySettings = 0x102b,
    FocusSettings = 0x102d,
    AFCSettings = 0x102e,
    SlowSync = 0x1030,
    PictureMode = 0x1031,
    ExposureCount = 0x1032,
    EXRAuto = 0x1033,
    EXRMode = 0x1034,
    ShadowTone = 0x1040,
    HighlightTone = 0x1041,
    DigitalZoom = 0x1044,
    LensModulationOptimizer = 0x1045,
    GrainEffect = 0x1047,
    ColorChromeEffect = 0x1048,
    BWAdjustment = 0x1049,
    CropMode = 0x104d,
    ColorChromeFXBlue = 0x104e,
    ShutterType = 0x1050,
    AutoBracketing = 0x1100,
    SequenceNumber = 0x1101,
    DriveSettings = 0x1103,
    PixelShiftShots = 0x1105,
    PixelShiftOffset = 0x1106,
    PanoramaAngle = 0x1153,
    PanoramaDirection = 0x1154,
    AdvancedFilter = 0x1201,
    ColorMode = 0x1210,
    BlurWarning = 0x1300,
    FocusWarning = 0x1301,
    ExposureWarning = 0x1302,
    GEImageSize = 0x1304,
    DynamicRange = 0x1400,
    FilmMode = 0x1401,
    DynamicRangeSetting = 0x1402,
    DevelopmentDynamicRange = 0x1403,
    MinFocalLength = 0x1404,
    MaxFocalLength = 0x1405,
    MaxApertureAtMinFocal = 0x1406,
    MaxApertureAtMaxFocal = 0x1407,
    AutoDynamicRange = 0x140b,
    ImageStabilization = 0x1422,
    SceneRecognition = 0x1425,
    Rating = 0x1431,
    ImageGeneration = 0x1436,
    ImageCount = 0x1438,
    DRangePriority = 0x1443,
    DRangePriorityAuto = 0x1444,
    DRangePriorityFixed = 0x1445,
    FlickerReduction = 0x1446,
    VideoRecordingMode = 0x3803,
    PeripheralLighting = 0x3804,
    VideoCompression = 0x3806,
    FrameRate = 0x3820,
    FrameWidth = 0x3821,
    FrameHeight = 0x3822,
    FullHDHighSpeedRec = 0x3824,
    FaceElementSelected = 0x4005,
    FacesDetected = 0x4100,
    FacePositions = 0x4103,
    NumFaceElements = 0x4200,
    FaceElementTypes = 0x4201,
    FaceElementPositions = 0x4203,
    FaceRecInfo = 0x4282,
    FileSource = 0x8000,
    OrderNumber = 0x8002,
    FrameNumber = 0x8003,
    Parallax = 0xb211,
}

/// These are only related to the additional FujiIFD in RAF files
#[derive(Debug, Copy, Clone, PartialEq, enumn::N, FromPrimitive)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum FujiIFD {
    FujiIFD = 0xf000,
    RawImageFullWidth = 0xf001,
    RawImageFullHeight = 0xf002,
    BitsPerSample = 0xf003,
    StripOffsets = 0xf007,
    StripByteCounts = 0xf008,
    BlackLevel = 0xf00a,
    GeometricDistortionParams = 0xf00b,
    WB_GRBLevelsStandard = 0xf00c,
    WB_GRBLevelsAuto = 0xf00d,
    WB_GRBLevels = 0xf00e,
    ChromaticAberrationParams = 0xf00f,
    VignettingParams = 0xf010,
}

/// These are only related to the additional RAF-tags in RAF files
// #[derive(Debug, Copy, Clone, PartialEq, enumn::N, FromPrimitive)]
// #[repr(u16)]
// #[allow(non_camel_case_types)]
// pub enum RafTags {
//     RawImageFullSize = 0x0100,
//     RawImageCropTopLeft = 0x0110,
//     RawImageCroppedSize = 0x0111,
//     RawImageAspectRatio = 0x0115,
//     RawImageSize = 0x0121,
//     FujiLayout = 0x0130,
//     XTransLayout = 0x0131,
//     WB_GRGBLevels = 0x2ff0,
//     RelativeExposure = 0x9200,
//     RawExposureBias = 0x9650,
//     RAFData = 0xc000,
// }

#[derive(Debug)]
pub struct FujiParser {
    pub jpeg_exif: Option<Exif>,
    pub raw_exif: Option<Exif>,
}

impl Default for FujiParser {
    fn default() -> Self {
        Self {
            jpeg_exif: None,
            raw_exif: None,
        }
    }
}

pub fn is_fuji_raf(buf: &[u8]) -> bool {
    buf[0..8] == b"FUJIFILM"[..]
}

impl FujiParser {
    pub fn parse<R>(&mut self, reader: &mut R) -> Result<(), Error>
    where
        R: io::BufRead + io::Seek,
    {
        self.jpeg_exif = extract_jpeg_exif(reader).ok();

        // let _ = self.parse_sub(data, marker::TIFF1_JPEG_PTR_OFFSET + 12);
        self.raw_exif = self.parse_fuji_raw(reader).ok();

        if let (Some(raw_exif), Some(jpeg_exif)) = (&self.raw_exif, &self.jpeg_exif) {
            Exif::merge_two_exif(raw_exif, jpeg_exif);
        }

        // IFD0 loading
        Ok(())
    }

    /// RAF format contains multiple TIFF and TIFF-like structures.
    /// This creates an IFD with all other IFD's found as sub IFD's.
    fn parse_fuji_raw<R>(&mut self, reader: &mut R) -> Result<Exif, Error>
    where
        R: BufRead + Seek,
    {
        reader.seek(std::io::SeekFrom::Start(
            marker::TIFF1_JPEG_PTR_OFFSET as u64,
        ))?;
        let ifd_offset = read32(reader).expect("Failed to read ifd_offset");

        reader.seek(std::io::SeekFrom::Start((ifd_offset + 12) as u64))?;
        // Main IFD
        let little_endian = match read16(reader) {
            Ok(0x4949) => true,
            Ok(0x4d4d) => false,
            _ => return Err(Error::NotFound("Invalid endian")),
        };

        reader.seek(std::io::SeekFrom::Start(marker::TIFF2_PTR_OFFSET as u64))?;

        // let second_ifd_offset = read32(reader).expect("Failed to read second_ifd_offset");

        // Read the primary RAF tags. JPEG exif tags will be read later.
        reader
            .seek(std::io::SeekFrom::Start(marker::TAGS_PTR_OFFSET as u64))
            .expect("Failed to seek to TAGS_PTR_OFFSET");

        let raf_offset = read32(reader).expect("Failed to read raf_offset");

        reader.seek(SeekFrom::Start(raf_offset as u64))?;
        let num_tags = read32(reader).expect("Failed to read num_tags");

        let mut entries = Vec::new();
        for _ in 0..num_tags {
            let tag_value = read16(reader).expect("Failed to read tag");
            let len = read16(reader).expect("Failed to read len") as usize;

            let tag = Tag(Context::FujiRaf, tag_value);

            match tag {
                Tag::RawImageFullSize
                | Tag::RawImageCropTopLeft
                | Tag::RawImageCroppedSize
                | Tag::RawImageAspectRatio
                | Tag::WB_GRGBLevels => {
                    let n = len / size_of::<u16>();

                    entries.push(IfdEntry {
                        field: Field {
                            tag: tag,
                            ifd_num: In::PRIMARY,
                            value: Value::Short(
                                (0..n)
                                    .map(|_| reader.read_u16::<byteorder::BigEndian>())
                                    .collect::<std::io::Result<Vec<_>>>()?,
                            ),
                        }
                        .into(),
                    });
                }
                Tag::FujiLayout | Tag::XTransLayout => {
                    let n = len / size_of::<u8>();
                    entries.push(IfdEntry {
                        field: Field {
                            tag: tag,
                            ifd_num: In::PRIMARY,
                            value: Value::Byte(
                                (0..n)
                                    .map(|_| reader.read_u8())
                                    .collect::<std::io::Result<Vec<_>>>()?,
                            ),
                        }
                        .into(),
                    });
                }
                // This one is in other byte-order...
                Tag::RAFData => {
                    let n = len / size_of::<u32>();
                    entries.push(IfdEntry {
                        field: Field {
                            tag: tag,
                            ifd_num: In::PRIMARY,
                            value: Value::Long(
                                (0..n)
                                    .map(|_| reader.read_u32::<byteorder::LittleEndian>())
                                    .collect::<std::io::Result<Vec<_>>>()?,
                            ),
                        }
                        .into(),
                    });
                }
                // Skip other tags
                _ => {
                    reader.seek(SeekFrom::Current(len as i64))?;
                }
            }
        }

        let entry_map: HashMap<(In, Tag), usize> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.ifd_num_tag(), i))
            .collect();

        let exif = Exif {
            buf: Vec::new(),
            entries,
            entry_map: entry_map,
            little_endian: little_endian,
        };

        // println!(
        //     "----------------------------- START FUJI RAF TAGS ---------------------------------"
        // );

        // for f in exif.fields() {
        //     println!(
        //         "  {}/{}: {}",
        //         f.ifd_num.index(),
        //         f.tag,
        //         f.display_value().with_unit(&exif)
        //     );
        //     println!("      {:?}", f.value);
        // }

        // println!(
        //     "----------------------------- END FUJI RAF TAGS ---------------------------------"
        // );

        Ok(exif)
    }
}

fn extract_jpeg_exif<R>(reader: &mut R) -> Result<Exif, Error>
where
    R: BufRead + Seek,
{
    let jpeg_thumbnail = extract_jpeg_thumbnail(reader)?;
    let jpeg_exif_offset_buffer = jpeg::get_exif_attr(&mut jpeg_thumbnail.chain(reader))?;
    let jpeg_exif = read_raw(jpeg_exif_offset_buffer)?;
    Ok(jpeg_exif)
}

fn extract_jpeg_thumbnail<R>(reader: &mut R) -> Result<Vec<u8>, Error>
where
    R: BufRead + Seek,
{
    // Read the Primary TIFF / JPEG Header which will parse almost all the common tags
    reader
        .seek(std::io::SeekFrom::Start(
            marker::TIFF1_JPEG_PTR_OFFSET as u64,
        ))
        .expect("Failed to seek to TIFF1_JPEG_PTR_OFFSET");

    let jpeg_offset = read32(reader).expect("Failed to read JPEG offset");
    let jpeg_length = read32(reader).expect("Failed to read JPEG length");

    reader
        .seek(std::io::SeekFrom::Start(jpeg_offset.into()))
        .expect("Failed to seek to JPEG offset");

    let mut embedded_jpeg = vec![0u8; jpeg_length as usize];
    reader
        .read_exact(&mut embedded_jpeg)
        .expect("Failed to read JPEG data");

    Ok(embedded_jpeg)
}

/// Parses the Exif attributes from raw Exif data.
/// If an error occurred, `exif::Error` is returned.
fn read_raw(data: Vec<u8>) -> Result<Exif, Error> {
    let mut parser = parser::Parser::default();
    parser.parse(&data)?;
    let entry_map = parser
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.ifd_num_tag(), i))
        .collect();
    Ok(Exif {
        buf: data,
        entries: parser.entries,
        entry_map: entry_map,
        little_endian: parser.little_endian,
    })
}
