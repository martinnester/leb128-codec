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

fn get_shr<N: num_traits::PrimInt>() -> fn(N, u32) -> N {
    if is_signed::<N>() {
        N::signed_shr
    } else {
        N::unsigned_shr
    }
}
fn is_signed<N: num_traits::PrimInt>() -> bool {
    return N::zero().checked_sub(&N::one()).is_some();
}
fn is_encode_end<N: num_traits::PrimInt>(num: N) -> bool {
    let shr = get_shr::<N>();
    if is_signed::<N>() {
        let num = shr(num, 6);
        num.is_zero() || (num + N::one()).is_zero()
    } else {
        let num = shr(num, 7);
        num.is_zero()
    }
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
        let byte_mask = N::from(0xFF).unwrap();
        let mut num = self;
        let mut bytes_written = 0;
        let shr = get_shr::<Self>();
        loop {
            let byte: u8 = (num & byte_mask).to_u8().unwrap();
            let ends = is_encode_end(num);
            num = shr(num, 7);
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

    use std::{
        cmp::min,
        fmt::Debug,
        io::{self, Write},
    };

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

    fn read_byte<R: io::Read>(reader: &mut R) -> Option<u8> {
        let mut byte = [0];
        if reader.read(&mut byte).unwrap() == 1 {
            Some(byte[0])
        } else {
            None
        }
    }

    fn assert_buffers_eq<const A: usize, const E: usize>(actual: [u8; A], expected: [u8; E]) {
        let mut expected_read = &expected[..];
        let mut actual_read = &actual[..];
        loop {
            match (read_byte(&mut actual_read), read_byte(&mut expected_read)) {
                (Some(byte_a), Some(byte_b)) => assert_eq!(byte_a, byte_b),
                (Some(0), None) | (None, Some(0)) | (None, None) => break,
                x => panic!("{x:?}"),
            }
        }
    }
    fn assert_trip_exact<N: PrimInt + Debug, const E: usize>(num: N, encoding: [u8; E]) {
        let mut buf = [0; 32];
        let mut writable = &mut buf[..];
        num.leb128_encode(&mut writable).unwrap();
        assert_buffers_eq(buf, encoding);
        let mut readable = &buf[..];
        assert_eq!(N::leb128_decode(&mut readable).unwrap(), num);
    }
    #[test]
    fn test_unsigned_exact() {
        assert_trip_exact(0x81u8, [0x81, 0x1]);
        assert_trip_exact(0x29442u64, [0xC2, 0xA8, 0xA]);
    }
    #[test]
    fn test_signed_exact() {
        assert_trip_exact(0x7Fi16, [0xFF, 0x00]);
        assert_trip_exact(-0x53i32, [0xAD, 0x7F]);
        assert_trip_exact(-0x8652i32, [0xAE, 0xF3, 0x7D]);
    }
}
