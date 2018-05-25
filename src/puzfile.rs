extern crate nom;

use nom::{le_u16, le_u8};

use std::str;

use encoding::all::ISO_8859_1;
use encoding::{Encoding, DecoderTrap};

#[derive(Debug)]
pub struct PuzFile {
    pub preamble: Vec<u8>,
    pub checksum: u16,
    pub magic: String,
    pub cib_checksum: u16,
    pub masked_low_checksum_1: u16,
    pub masked_low_checksum_2: u16,
    pub masked_high_checksum_1: u16,
    pub masked_high_checksum_2: u16,
    pub version: String,
    pub reserved_1: u16,
    pub scrambled_checksum: u16,
    pub reserved_2: Vec<u8>,
    pub width: u8,
    pub height: u8,
    pub num_clues: u16,
    pub unknown_bitmask: u16,
    pub scrambled: u16,
    pub puzzle: String,
    pub state: String,
    pub title: String,
    pub author: String,
    pub copyright: String,
    pub clues: Vec<String>,
    pub notes: String,
}

named!(null_string_ascii<&[u8], String>,
   do_parse!(
       s: take_until!("\0") >>
       take!(1) >>
       ( ISO_8859_1.decode(s, DecoderTrap::Ignore).unwrap() )
   )
);

named!(checksum, terminated!(take!(2), peek!(tag!("ACROSS&DOWN"))));

named!(pub parse_all<&[u8], PuzFile>,
    do_parse!(
        preamble: opt!(many_till!(take!(1), peek!(checksum))) >>
        checksum: flat_map!(checksum, le_u16) >>
        magic: null_string_ascii >>
        cib_checksum: le_u16 >>
        masked_low_checksum_1: le_u16 >>
        masked_low_checksum_2: le_u16 >>
        masked_high_checksum_1: le_u16 >>
        masked_high_checksum_2: le_u16 >>
        version: map_res!(take!(4), str::from_utf8) >> 
        reserved_1: le_u16 >>
        scrambled_checksum: le_u16 >>
        reserved_2: take!(12) >>
        width: le_u8 >>
        height: le_u8 >>
        num_clues: le_u16 >>
        unknown_bitmask: le_u16 >>
        scrambled: le_u16 >>
        puzzle: map_res!(take!(width as usize * height as usize), str::from_utf8) >>
        state: map_res!(take!(width as usize * height as usize), str::from_utf8) >>
        title: null_string_ascii >>
        author: null_string_ascii >>
        copyright: null_string_ascii >>
        clues: many_m_n!(num_clues as usize, num_clues as usize, null_string_ascii) >>
        notes: null_string_ascii >>
        (PuzFile {
            preamble: match preamble {
                Some(p) => p.0.iter().map(|x| x[0]).collect(),
                _ => Vec::new()
            },
            checksum: checksum,
            magic: magic,
            cib_checksum: cib_checksum,
            masked_low_checksum_1: masked_low_checksum_1,
            masked_low_checksum_2: masked_low_checksum_2,
            masked_high_checksum_1: masked_high_checksum_1,
            masked_high_checksum_2: masked_high_checksum_2,
            version: version.into(),
            reserved_1: reserved_1,
            scrambled_checksum: scrambled_checksum,
            reserved_2: reserved_2.into(),
            width: width,
            height: height,
            num_clues: num_clues,
            unknown_bitmask: unknown_bitmask,
            scrambled: scrambled,
            puzzle: puzzle.into(),
            state: state.into(),
            title: title,
            author: author,
            copyright: copyright,
            clues: clues,
            notes: notes
        })
    )
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let d = include_bytes!("../assets/test1.puz");
        let p = match parse_all(d) {
            Ok((_, p)) => p,
            Err(Err::Incomplete(x)) => panic!("incomplete: {:?}", x),
            Err(Err::Error(e)) => panic!("error: {:?}", e),
            Err(Err::Failure(e)) => panic!("failure: {:?}", e)
        };

        assert_eq!(p.width, 15);
        assert_eq!(p.height, 15);
        assert_eq!(
            p.puzzle,
            concat!(
                "TEST.ZEBRA.TEST",
                "TEST.ZEBRA.TEST",
                "MUDSKIPPER.TEST",
                "PUZ.TEST.WIZARD",
                "...TEST.GIZZARD",
                "WIZARD.WIZARD..",
                "ZEBRA.ZEBRA.PUZ",
                "TEST.ZEBRA.TEST",
                "PUZ.ZEBRA.ZEBRA",
                "..WIZARD.WIZARD",
                "GIZZARD.TEST...",
                "WIZARD.TEST.PUZ",
                "TEST.MUDSKIPPER",
                "TEST.ZEBRA.TEST",
                "TEST.ZEBRA.TEST"
            )
        );
        assert_eq!(p.title, "Test: Test Test Test");
        assert_eq!(p.author, "Created by Test");
        assert_eq!(p.copyright, "2017 Test");
        assert_eq!(p.num_clues, 78);
        assert_eq!(p.clues[0], "Test Across 1");
        assert_eq!(p.clues[77], "Test Across 39");
    }

    #[test]
    fn small() {
        let d = include_bytes!("../assets/test2.puz");
        let p = match parse_all(d) {
            Ok((_, p)) => p,
            Err(Err::Incomplete(x)) => panic!("incomplete: {:?}", x),
            Err(Err::Error(e)) => panic!("error: {:?}", e),
            Err(Err::Failure(e)) => panic!("failure: {:?}", e)
        };

        assert_eq!(p.width, 3);
        assert_eq!(p.height, 3);
        assert_eq!(p.puzzle, concat!("PUZ", "O.O", "POO"));
        assert_eq!(p.num_clues, 4);
    }

    #[test]
    fn leading_garbage() {
        let d = include_bytes!("../assets/test3.puz");
        let p = match parse_all(d) {
            Ok((_, p)) => p,
            Err(Err::Incomplete(x)) => panic!("incomplete: {:?}", x),
            Err(Err::Error(e)) => panic!("error: {:?}", e),
            Err(Err::Failure(e)) => panic!("failure: {:?}", e)
        };

        assert_eq!(p.width, 3);
        assert_eq!(p.height, 3);
        assert_eq!(p.puzzle, concat!("PUZ", "O.O", "POO"));
        assert_eq!(p.num_clues, 4);
    }

    #[test]
    fn rectangle() {
        let d = include_bytes!("../assets/test4.puz");
        let p = match parse_all(d) {
            Ok((_, p)) => p,
            Err(Err::Incomplete(x)) => panic!("incomplete: {:?}", x),
            Err(Err::Error(e)) => panic!("error: {:?}", e),
            Err(Err::Failure(e)) => panic!("failure: {:?}", e)
        };

        assert_eq!(p.width, 2);
        assert_eq!(p.height, 3);
        assert_eq!(p.puzzle, concat!("PA", "O.", "P."));
        assert_eq!(p.num_clues, 2);
    }
}
