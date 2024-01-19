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

fn is_signed<N: num_traits::PrimInt>() -> bool {
    return N::zero().checked_sub(&N::one()).is_some();
}

impl<N: num_traits::PrimInt> LEB128Codec for N {
    fn leb128_decode<R>(reader: &mut R) -> Result<Self, io::Error>
    where
        R: Sized + io::Read,
        Self: Sized,
    {
        if is_signed::<Self>() {
            todo!()
        } else {
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
    }

    fn leb128_encode<W>(self, writer: &mut W) -> Result<usize, io::Error>
    where
        W: Sized + io::Write,
        Self: Sized,
    {
        if is_signed::<Self>() {
            todo!();
        } else {
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
}

#[cfg(test)]
mod tests {

    use std::{cmp::min, io};

    use num_traits::PrimInt;

    use crate::LEB128Codec;

    fn trip<N: PrimInt + std::fmt::Debug, O: PrimInt + std::fmt::Debug>(
        num: N,
    ) -> Result<O, io::Error> {
        let mut buf = [0; 32];
        let mut writable = &mut buf[..];
        num.leb128_encode(&mut writable)?;
        let mut readable = &buf[..];
        O::leb128_decode(&mut readable)
    }

    fn assert_trip<N: PrimInt + std::fmt::Debug>(num: N) {
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
    fn signed_trips() {
        for x in -128..128 {
            assert_trip(x as i8);
        }
        for x in -32768..32768 {
            assert_trip(x as i16);
            assert_trip(x as i32 * 65536);
            assert_trip(x as i64 * 65536 * 65536);
            assert_trip(x as i128 * 65536 * 65536 * 65536);
        }
    }

    fn assert_trip_overflow<
        Encode: PrimInt + std::fmt::Debug,
        Decode: PrimInt + std::fmt::Debug,
    >(
        input: Encode,
    ) {
        assert!(trip::<Encode, Decode>(input).unwrap_err().kind() == io::ErrorKind::InvalidData)
    }

    fn test_overflow<Encode: PrimInt + std::fmt::Debug, Decode: PrimInt + std::fmt::Debug>(
        negative: bool,
    ) {
        let sign = if negative { -1 } else { 1 };
        let bit_size = Decode::zero().count_zeros();
        let first_overflow: i128 = 2.pow(bit_size) * sign;
        for x in 0..min(65536 * sign, first_overflow << 3) {
            assert_trip_overflow::<Encode, Decode>(Encode::from(first_overflow + x).unwrap());
        }
    }
    #[test]
    fn unsigned_overflow() {
        test_overflow::<u16, u8>(false);
        test_overflow::<u32, u16>(false);
        test_overflow::<u64, u32>(false);
        test_overflow::<u128, u64>(false);
    }
    #[test]
    fn signed_overflow() {
        test_overflow::<i16, i8>(false);
        test_overflow::<i32, i16>(false);
        test_overflow::<i64, i32>(false);
        test_overflow::<i128, i64>(false);

        test_overflow::<i16, i8>(true);
        test_overflow::<i32, i16>(true);
        test_overflow::<i64, i32>(true);
        test_overflow::<i128, i64>(true);
    }
}
