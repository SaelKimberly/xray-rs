#![allow(dead_code)]
use std::mem::MaybeUninit;

use blake2::{
    Blake2b,
    digest::consts::U32,
    digest::{FixedOutput, Update},
};
use rand::Rng;

const SM_PSK_MIN_LEN: usize = 4;
const SM_SALT_LEN: usize = 8;
const SM_KEY_LEN: usize = 32;

pub struct Salamander<R: Rng> {
    buf: Box<[MaybeUninit<u8>]>,
    rng: R,
}

impl Salamander<rand::rngs::ThreadRng> {
    #[inline]
    pub fn new_thread(psk: &[u8]) -> std::io::Result<Self> {
        Salamander::new(psk, rand::rng())
    }
}

impl<R: Rng> Salamander<R> {
    pub fn new<S: AsRef<[u8]>>(psk: S, rng: R) -> std::io::Result<Self> {
        let psk = psk.as_ref();
        let mut buf = Box::new_uninit_slice(psk.len() + SM_SALT_LEN);
        buf[..psk.len()].write_copy_of_slice(psk);

        if psk.len() >= SM_PSK_MIN_LEN {
            Ok(Self { buf, rng })
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "psk too short",
            ))
        }
    }

    #[inline]
    fn key(&mut self, salt: &[u8]) -> wide::u8x32 {
        let offset = self.buf.len() - SM_SALT_LEN;
        self.buf[offset..].write_copy_of_slice(salt);

        wide::u8x32::new(
            <Blake2b<U32> as blake2::Digest>::new()
                .chain(unsafe { self.buf.assume_init_ref() })
                .finalize_fixed()
                .into(),
        )
    }

    pub fn obfuscate(&mut self, in_data: &[u8], out_data: &mut [u8]) -> usize {
        let out_len = SM_SALT_LEN + in_data.len();
        if out_data.len() < out_len {
            return 0;
        }
        let (salt, out_data) = unsafe { out_data[..out_len].split_at_mut_unchecked(SM_SALT_LEN) };

        self.rng.fill_bytes(salt);

        let key = self.key(salt);

        let (i_data, i_suff) = in_data.as_chunks::<SM_KEY_LEN>();
        let (o_data, o_suff) = out_data.as_chunks_mut::<SM_KEY_LEN>();

        for (i, o) in i_data
            .iter()
            .copied()
            .map(wide::u8x32::new)
            .zip(o_data.iter_mut())
        {
            o.copy_from_slice((i ^ key).as_array());
        }

        for (k, (i, o)) in key
            .to_array()
            .iter()
            .zip(i_suff.iter().zip(o_suff.iter_mut()))
        {
            *o = *k ^ *i;
        }

        out_len
    }

    pub fn deobfuscate(&mut self, in_data: &[u8], out_data: &mut [u8]) -> usize {
        if in_data.len() <= SM_SALT_LEN {
            return 0;
        }
        let out_len = in_data.len() - SM_SALT_LEN;
        if out_len > out_data.len() {
            return 0;
        }

        let (salt, in_data) = unsafe { in_data.split_at_unchecked(SM_SALT_LEN) };
        let out_data = &mut out_data[..out_len];

        let key = self.key(salt);

        let (i_data, i_suff) = in_data.as_chunks::<SM_KEY_LEN>();
        let (o_data, o_suff) = out_data.as_chunks_mut::<SM_KEY_LEN>();

        for (i, o) in i_data
            .iter()
            .copied()
            .map(wide::u8x32::new)
            .zip(o_data.iter_mut())
        {
            o.copy_from_slice((i ^ key).as_array());
        }

        for (k, (i, o)) in key
            .to_array()
            .iter()
            .zip(i_suff.iter().zip(o_suff.iter_mut()))
        {
            *o = *k ^ *i;
        }

        out_len
    }
}

#[cfg(test)]
mod tests {
    use rand::RngExt;

    use super::*;

    #[test]
    fn test_salamander() {
        let mut rng = rand::rngs::ThreadRng::default();
        let mut salamander = Salamander::new_thread(b"password").unwrap();

        let mut in_data = [0u8; 1200];
        let mut o_out = [0u8; 2048];
        let mut d_out = [0u8; 2048];

        let in_data = &mut in_data[..];
        let o_out = &mut o_out[..];
        let d_out = &mut d_out[..];

        for _ in 0..1000 {
            rng.fill(in_data);

            let n = salamander.obfuscate(in_data, o_out);
            assert_eq!(n, in_data.len() + SM_SALT_LEN);

            let n = salamander.deobfuscate(&o_out[..n], d_out);
            assert_eq!(n, in_data.len());

            assert_eq!(in_data, &d_out[..n]);
        }
    }
}
