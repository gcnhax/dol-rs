extern crate byteorder;
#[macro_use]
extern crate itertools;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use std::io::{self, Read, Seek, SeekFrom, Write};

mod error;
pub use error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Text,
    Data,
}

#[derive(Debug)]
pub struct Section {
    pub kind: SectionKind,
    pub address: u32,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct DolFile {
    pub sections: Vec<Section>,
    pub bss_start: u32,
    pub bss_length: u32,
    pub entry_point: u32,
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
    pub fn parse<R>(rdr: &mut R) -> Result<Self, Error>
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

    pub fn write<W>(&self, wtr: &mut W) -> Result<(), Error>
    where
        W: Write,
    {
        for &offset in &self.section_offsets {
            wtr.write_u32::<BE>(offset)?;
        }

        for &address in &self.section_addresses {
            wtr.write_u32::<BE>(address)?;
        }

        for &length in &self.section_lengths {
            wtr.write_u32::<BE>(length)?;
        }

        wtr.write_u32::<BE>(self.bss_start)?;
        wtr.write_u32::<BE>(self.bss_length)?;
        wtr.write_u32::<BE>(self.entry_point)?;

        // padding
        wtr.write_all(&[0; 0x1c])?;

        Ok(())
    }
}

fn load_sections<R>(
    rdr: &mut R,
    offsets: &[u32],
    addresses: &[u32],
    lengths: &[u32],
    kind: SectionKind,
) -> Result<Vec<Section>, Error>
where
    R: Read + Seek,
{
    izip!(offsets, addresses, lengths)
        .filter(|(_, _, &l)| l > 0)
        .map(|(&offset, &address, &length)| {
            let mut data = Vec::with_capacity(length as usize);

            rdr.seek(SeekFrom::Start(offset as u64))?;
            rdr.take(length as u64).read_to_end(&mut data)?;

            Ok(Section {
                kind,
                address,
                data,
            })
        })
        .collect()
}

impl DolFile {
    pub fn parse<R>(rdr: &mut R) -> Result<Self, Error>
    where
        R: Read + Seek,
    {
        let header = DolHeader::parse(rdr)?;
        let mut sections = load_sections(
            rdr,
            &header.section_offsets[0..7],
            &header.section_addresses[0..7],
            &header.section_addresses[0..7],
            SectionKind::Text,
        )?;
        sections.extend(load_sections(
            rdr,
            &header.section_offsets[7..18],
            &header.section_addresses[7..18],
            &header.section_addresses[7..18],
            SectionKind::Data,
        )?);

        Ok(DolFile {
            sections,
            bss_start: header.bss_start,
            bss_length: header.bss_length,
            entry_point: header.entry_point,
        })
    }

    pub fn write<W>(&self, wtr: &mut W) -> Result<(), Error>
    where
        W: Write + Seek,
    {
        let text_sections: Vec<_> = self.sections
            .iter()
            .filter(|s| s.kind == SectionKind::Text)
            .collect();
        let data_sections: Vec<_> = self.sections
            .iter()
            .filter(|s| s.kind == SectionKind::Data)
            .collect();

        if text_sections.len() > 7 {
            return Err(Error::TooManySections(SectionKind::Text));
        }

        if data_sections.len() > 11 {
            return Err(Error::TooManySections(SectionKind::Data));
        }

        let mut section_lengths: [u32; 18] = [0; 18];
        let mut section_addresses: [u32; 18] = [0; 18];
        let mut section_offsets: [u32; 18] = [0; 18];

        let mut current_offset = 0x100;
        let mut section_queue = Vec::new();

        for (i, section) in text_sections.iter().enumerate() {
            section_lengths[0..7][i] = section.data.len() as u32;
            section_addresses[0..7][i] = section.address;
            section_offsets[0..7][i] = current_offset;

            section_queue.push((current_offset, section));
            current_offset += section.data.len() as u32 + 5;
        }

        for (i, section) in data_sections.iter().enumerate() {
            section_lengths[7..18][i] = section.data.len() as u32;
            section_addresses[7..18][i] = section.address;
            section_offsets[7..18][i] = current_offset;

            section_queue.push((current_offset, section));
            current_offset += section.data.len() as u32;
        }

        let header = DolHeader {
            bss_start: self.bss_start,
            bss_length: self.bss_length,
            entry_point: self.entry_point,
            section_lengths: section_lengths,
            section_addresses: section_addresses,
            section_offsets: section_offsets,
        };

        header.write(wtr)?;

        for (offset, section) in section_queue {
            let current_position = wtr.seek(SeekFrom::Current(0))?;
            // write padding in run to next offset
            for _ in 0..(offset as u64 - current_position) {
                wtr.write_all(&[0])?
            }
            // write the data
            wtr.write_all(&section.data)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn too_many_text_sections() {
        use std::io::Cursor;
        let mut cur = Cursor::new(Vec::new());

        let mut sections = Vec::new();
        for _ in 0..8 {
            sections.push(Section {address: 0x10, data: vec![1,3,3,7], kind: SectionKind::Text});
        }

        let hdr = DolFile {
            sections,
            bss_start: 0, bss_length: 0,
            entry_point: 0x10,
        };

        assert!(hdr.write(&mut cur).is_err(), "attempting to write too many text sections should cause an error");
    }

    #[test]
    fn too_many_data_sections() {
        use std::io::Cursor;
        let mut cur = Cursor::new(Vec::new());
        let mut sections = Vec::new();

        for _ in 0..12 {
            sections.push(Section {address: 0x10, data: vec![1,3,3,7], kind: SectionKind::Data});
        }

        let hdr = DolFile {
            sections,
            bss_start: 0, bss_length: 0,
            entry_point: 0x10,
        };

        assert!(hdr.write(&mut cur).is_err(), "attempting to write too many data sections should cause an error");
    }

    #[test]
    fn write_dol_header() {
        use std::io::Cursor;
        let mut cur = Cursor::new(Vec::new());

        let hdr = DolHeader {
            section_offsets: [
                256, 265, 274, 283, 292, 301, 310, 319, 324, 329, 334, 339, 344, 349, 354, 359,
                364, 369,
            ],
            section_addresses: [
                16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
            ],
            section_lengths: [4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
            bss_start: 0,
            bss_length: 0,
            entry_point: 16,
        };

        hdr.write(&mut cur).unwrap();

        let expected: &[u8] = &[
            0, 0, 1, 0, 0, 0, 1, 9, 0, 0, 1, 18, 0, 0, 1, 27, 0, 0, 1, 36, 0, 0, 1, 45, 0, 0, 1,
            54, 0, 0, 1, 63, 0, 0, 1, 68, 0, 0, 1, 73, 0, 0, 1, 78, 0, 0, 1, 83, 0, 0, 1, 88, 0, 0,
            1, 93, 0, 0, 1, 98, 0, 0, 1, 103, 0, 0, 1, 108, 0, 0, 1, 113, 0, 0, 0, 16, 0, 0, 0, 16,
            0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0,
            16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0,
            0, 16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0, 4, 0, 0, 0, 4, 0, 0, 0, 4, 0, 0, 0, 4, 0, 0,
            0, 4, 0, 0, 0, 4, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0, 5, 0, 0, 0, 5, 0, 0, 0, 5, 0, 0, 0,
            5, 0, 0, 0, 5, 0, 0, 0, 5, 0, 0, 0, 5, 0, 0, 0, 5, 0, 0, 0, 5, 0, 0, 0, 5, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0,
        ];

        assert_eq!(cur.into_inner(), expected);
    }

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
