use std::io::{BufRead, BufReader, ErrorKind};

use crate::error::Error;
use crate::tiff::IfdEntry;
use crate::util::{read16, read32, read8, BufReadExt as _, ReadExt as _};
use crate::{Field, Tag};

use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::collections::BTreeMap;
use std::f32::NAN;
use std::io::Cursor;
use std::io::SeekFrom;
use std::mem::size_of;

use std::io::{self, Seek};

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

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
#[derive(Debug, Copy, Clone, PartialEq, enumn::N, FromPrimitive)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum RafTags {
    RawImageFullSize = 0x0100,
    RawImageCropTopLeft = 0x0110,
    RawImageCroppedSize = 0x0111,
    RawImageAspectRatio = 0x0115,
    RawImageSize = 0x0121,
    FujiLayout = 0x0130,
    XTransLayout = 0x0131,
    WB_GRGBLevels = 0x2ff0,
    RelativeExposure = 0x9200,
    RawExposureBias = 0x9650,
    RAFData = 0xc000,
}

pub fn is_fuji_raf(buf: &[u8]) -> bool {
    buf[0..8] == b"FUJIFILM"[..]
}

pub fn parse_fuji_raw<R>(reader: &mut R) -> Result<Vec<u8>, Error>
where
    R: BufRead + Seek,
{
    // Read until the first pointer offset
    // reader
    //     .discard_exact(marker::TIFF1_JPEG_PTR_OFFSET)
    //     .expect_err(&format!(
    //         "Expected to read {} bytes from RAF file",
    //         marker::TIFF1_JPEG_PTR_OFFSET
    //     ));
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

    let mut buf = vec![0u8; jpeg_length as usize];
    reader
        .read_exact(&mut buf)
        .expect("Failed to read JPEG data");

    Ok(buf)
}

// /// Get the Exif attribute information segment from a JPEG file.
// pub fn parse_fuji_raw_old<R>(reader: &mut R)
// where
//     R: BufRead,
// {
//     let mut bytes_to_read = [0u8; marker::TIFF1_PTR_OFFSET];

//     let _ = reader.read_exact(&mut bytes_to_read);

//     let num_ifd_entries = read32(reader).ok().unwrap();

//     println!("num_ifd_entries: {:?}", num_ifd_entries);

//     for _ in 0..num_ifd_entries {
//         let tag_code = read16(reader).unwrap();
//         let len = read16(reader).unwrap_or(0) as usize;
//         if let Some(tag) = RafTags::from_u16(tag_code) {
//             match tag {
//                 RafTags::RawImageFullSize
//                 | RafTags::RawImageCropTopLeft
//                 | RafTags::RawImageCroppedSize
//                 | RafTags::RawImageAspectRatio
//                 | RafTags::WB_GRGBLevels => {
//                     println!("tag: {:?}, len: {}", tag, len);
//                     // let n = len / size_of::<u16>();
//                     // let entry = Entry {
//                     //     tag,
//                     //     value: Value::Short(
//                     //         (0..n)
//                     //             .map(|_| stream.read_u16::<BigEndian>())
//                     //             .collect::<std::io::Result<Vec<_>>>()?,
//                     //     ),
//                     //     embedded: None,
//                     // };
//                     // entries.insert(tag, entry);
//                 }
//                 RafTags::FujiLayout | RafTags::XTransLayout => {
//                     println!("tag: {:?}, len: {}", tag, len);
//                     // let n = len / size_of::<u8>();
//                     // let entry = Entry {
//                     //     tag,
//                     //     value: Value::Byte(
//                     //         (0..n)
//                     //             .map(|_| stream.read_u8())
//                     //             .collect::<std::io::Result<Vec<_>>>()?,
//                     //     ),
//                     //     embedded: None,
//                     // };
//                     // entries.insert(tag, entry);
//                 }
//                 // This one is in other byte-order...
//                 RafTags::RAFData => {
//                     println!("tag: {:?}, len: {}", tag, len);
//                     // let n = len / size_of::<u32>();
//                     // let entry = Entry {
//                     //     tag,
//                     //     value: Value::Long(
//                     //         (0..n)
//                     //             .map(|_| stream.read_u32::<LittleEndian>())
//                     //             .collect::<std::io::Result<Vec<_>>>()?,
//                     //     ),
//                     //     embedded: None,
//                     // };
//                     // entries.insert(tag, entry);
//                 }
//                 // Skip other tags
//                 _ => {
//                     // stream.seek(SeekFrom::Current(len as i64))?;
//                     println!("tag: {:?}, len: {}", tag, len);
//                 }
//             }
//         }
//     }
// }
