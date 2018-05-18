extern crate byteorder;
#[macro_use]
extern crate itertools;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
use byteorder::{ReadBytesExt, BE};
use std::io::{self, Read, Seek, SeekFrom};

#[derive(Debug)]
pub struct Section {
    pub address: u32,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct DolFile {
    pub text_sections: Vec<Section>,
    pub data_sections: Vec<Section>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DolHeader {
    pub section_offsets: [u32; 18],
    pub section_addresses: [u32; 18],
    pub section_lengths: [u32; 18],
    pub bss_start: u32,
    pub bss_length: u32,
    pub entry_point: u32,
}

impl DolHeader {
    pub fn parse<R>(rdr: &mut R) -> Result<DolHeader, io::Error>
    where
        R: Read + Seek,
    {
        let mut section_offsets: [u32; 18] = [0; 18];
        let mut section_addresses: [u32; 18] = [0; 18];
        let mut section_lengths: [u32; 18] = [0; 18];
        rdr.read_u32_into::<BE>(&mut section_offsets)?;
        rdr.read_u32_into::<BE>(&mut section_addresses)?;
        rdr.read_u32_into::<BE>(&mut section_lengths)?;

        let bss_start = rdr.read_u32::<BE>()?;
        let bss_length = rdr.read_u32::<BE>()?;
        let entry_point = rdr.read_u32::<BE>()?;

        // padding
        rdr.seek(SeekFrom::Current(0x1c))?;

        Ok(DolHeader {
            section_offsets,
            section_addresses,
            section_lengths,
            bss_start,
            bss_length,
            entry_point,
        })
    }
}

fn load_sections<R>(
    rdr: &mut R,
    offsets: &[u32],
    addresses: &[u32],
    lengths: &[u32],
) -> Result<Vec<Section>, io::Error>
where
    R: Read + Seek,
{
    izip!(offsets, addresses, lengths)
        .filter(|(_, _, &l)| l > 0)
        .map(|(&offset, &address, &length)| {
            let mut data = Vec::with_capacity(length as usize);

            rdr.seek(SeekFrom::Start(offset as u64))?;
            rdr.take(length as u64).read_to_end(&mut data)?;

            Ok(Section { address, data })
        })
        .collect()
}

impl DolFile {
    pub fn parse<R>(rdr: &mut R) -> Result<DolFile, io::Error>
    where
        R: Read + Seek,
    {
        let header = DolHeader::parse(rdr)?;
        let text_sections = load_sections(
            rdr,
            &header.section_offsets[0..=6],
            &header.section_addresses[0..=6],
            &header.section_addresses[0..=6],
        )?;
        let data_sections = load_sections(
            rdr,
            &header.section_offsets[6..=17],
            &header.section_addresses[6..=17],
            &header.section_addresses[6..=17],
        )?;

        Ok(DolFile {
            text_sections,
            data_sections,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_dol_header() {
        use std::fs::File;
        let mut f = File::open("data/metronome.dol").unwrap();

        let hdr = DolHeader::parse(&mut f).unwrap();

        assert_eq!(
            hdr,
            DolHeader {
                section_offsets: [0x100, 0x8e0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                section_addresses: [
                    0x8026caa0, 0x8026d280, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                ],
                section_lengths: [0x7e0, 0xd4d00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                bss_start: 0,
                bss_length: 0,
                entry_point: 0x8026caa0,
            }
        );
    }
}
