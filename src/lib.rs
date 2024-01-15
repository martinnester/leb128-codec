use std::io;
pub trait LEB128Codec {
    fn leb128_decode<R>(reader: &mut R) -> Result<Self, io::Error>
    where
        R: Sized + io::Read,
        Self: Sized;
    fn leb128_encode<W>(self, writer: &mut W) -> Result<usize, io::Error>
    where
        W: Sized + io::Write,
        Self: Sized;
}

pub const CONTINUATION: u8 = 1 << 7;

impl<N: num_traits::Unsigned + num_traits::PrimInt> LEB128Codec for N {
    fn leb128_decode<R>(reader: &mut R) -> Result<Self, io::Error>
    where
        R: Sized + io::Read,
        Self: Sized,
    {
        let mut num = N::zero();
        let max_shift = ((num.count_zeros() as usize) / 7) * 7;
        let max_last_byte = !(0xFF << (num.count_zeros() as usize - max_shift));
        let mut buffer: [u8; 1] = [0];
        let mut shift = 0;
        loop {
            reader.read_exact(&mut buffer)?;
            let ends = (buffer[0] & CONTINUATION) == 0;
            if !ends {
                buffer[0] = buffer[0] ^ CONTINUATION;
            }
            let num_like: N = N::from(buffer[0]).unwrap();

            if shift == max_shift && buffer[0] > max_last_byte {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            }
            num = num | (num_like << shift);
            shift += 7;
            if ends {
                break Ok(num);
            }
        }
    }

    fn leb128_encode<W>(self, writer: &mut W) -> Result<usize, io::Error>
    where
        W: Sized + io::Write,
        Self: Sized,
    {
        let byte_mask = N::from(0xFF).unwrap();
        let mut num = self;
        let mut bytes_written = 0;
        loop {
            let byte: u8 = (num & byte_mask).to_u8().unwrap();
            num = num >> 7;
            let ends = num.is_zero();
            let out = if ends {
                byte & !CONTINUATION
            } else {
                byte | CONTINUATION
            };
            writer.write(&[out])?;
            bytes_written += 1;
            if ends {
                break Ok(bytes_written);
            };
        }
    }
}
#[cfg(test)]
mod tests {

    use std::{cmp::min, io};

    use num_traits::{PrimInt, Unsigned};

    use crate::LEB128Codec;

    fn trip<N: PrimInt + Unsigned + std::fmt::Debug, O: PrimInt + Unsigned + std::fmt::Debug>(
        num: N,
    ) -> Result<O, io::Error> {
        let mut buf = [0; 32];
        let mut writable = &mut buf[..];
        num.leb128_encode(&mut writable)?;
        let mut readable = &buf[..];
        O::leb128_decode(&mut readable)
    }

    fn assert_trip<N: PrimInt + Unsigned + std::fmt::Debug>(num: N) {
        assert_eq!(
            num,
            trip(num).unwrap_or_else(|e| panic!(
                "{:?} on {:?} of type u{:?}",
                e,
                num,
                N::zero().count_zeros()
            ))
        );
    }

    #[test]
    fn unsigned_trips() {
        for x in 0..256 {
            assert_trip(x as u8);
        }
        for x in 0..65536 {
            assert_trip(x as u16);
            assert_trip(x as u32 * 65536);
            assert_trip(x as u64 * 65536 * 65536);
            assert_trip(x as u128 * 65536 * 65536 * 65536);
        }
    }

    #[test]
    fn unsigned_overflow() {
        fn assert_trip_overflow<
            Encode: PrimInt + Unsigned + std::fmt::Debug,
            Decode: PrimInt + Unsigned + std::fmt::Debug,
        >(
            input: Encode,
        ) {
            assert!(trip::<Encode, Decode>(input).unwrap_err().kind() == io::ErrorKind::InvalidData)
        }

        fn test_overflow<
            Encode: PrimInt + Unsigned + std::fmt::Debug,
            Decode: PrimInt + Unsigned + std::fmt::Debug,
        >() {
            let bit_size = Decode::zero().count_zeros();
            let first_overflow: u128 = 2.pow(bit_size);
            for x in 0..min(65536, first_overflow << 3) {
                assert_trip_overflow::<Encode, Decode>(Encode::from(first_overflow + x).unwrap());
            }
        }

        test_overflow::<u16, u8>();
        test_overflow::<u32, u16>();
        test_overflow::<u64, u32>();
        test_overflow::<u128, u64>();
    }
}
