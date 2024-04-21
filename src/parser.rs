use crate::{ifd::IfdEntry, Error};

#[derive(Debug)]
pub struct Parser {
    pub entries: Vec<IfdEntry>,
    pub little_endian: bool,
}

pub trait Parse {
    fn parse(&mut self, data: &[u8]) -> Result<(), Error>;
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            little_endian: false,
        }
    }
}
