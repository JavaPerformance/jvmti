//! Java Modified UTF-8 encoding and decoding.
//!
//! JNI, JVM TI, and `CONSTANT_Utf8` class-file entries use Modified UTF-8,
//! not standard UTF-8. In particular, U+0000 is encoded as `C0 80`, and
//! supplementary Unicode characters are encoded as two three-byte surrogate
//! code units. Use the UTF-16 APIs when unpaired Java surrogates must be
//! preserved exactly.

use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::fmt;

/// The reason a Modified UTF-8 byte sequence could not be decoded exactly.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Mutf8ErrorKind {
    EmbeddedNul,
    UnexpectedEnd,
    InvalidLeadingByte,
    InvalidContinuationByte,
    OverlongEncoding,
    UnpairedSurrogate,
    InvalidScalarValue,
}

/// A precise Modified UTF-8 decoding failure.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mutf8Error {
    offset: usize,
    kind: Mutf8ErrorKind,
}

impl Mutf8Error {
    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn kind(self) -> Mutf8ErrorKind {
        self.kind
    }
}

impl fmt::Display for Mutf8Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid Modified UTF-8 at byte {}: {:?}",
            self.offset, self.kind
        )
    }
}

impl std::error::Error for Mutf8Error {}

/// Encode a Rust string as Java Modified UTF-8 without a trailing NUL.
pub fn encode(value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    encode_units(&mut out, value.encode_utf16());
    out
}

/// Encode exact Java UTF-16 code units as Modified UTF-8.
///
/// Unlike [`encode`], this accepts unpaired surrogate code units so JNI and
/// class-file tooling can preserve all Java string values.
pub fn encode_utf16(value: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len().saturating_mul(3));
    encode_units(&mut out, value.iter().copied());
    out
}

fn encode_units(out: &mut Vec<u8>, units: impl IntoIterator<Item = u16>) {
    for unit in units {
        match unit {
            0 => out.extend_from_slice(&[0xc0, 0x80]),
            0x0001..=0x007f => out.push(unit as u8),
            0x0080..=0x07ff => {
                out.push((0xc0 | (unit >> 6)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
            _ => {
                out.push((0xe0 | (unit >> 12)) as u8);
                out.push((0x80 | ((unit >> 6) & 0x3f)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
        }
    }
}

/// Encode a Rust string as a NUL-terminated Java Modified UTF-8 string.
pub fn encode_cstring(value: &str) -> CString {
    // Modified UTF-8 encodes U+0000 as C0 80, so `encode` cannot emit an
    // interior zero byte.
    unsafe { CString::from_vec_unchecked(encode(value)) }
}

fn error(offset: usize, kind: Mutf8ErrorKind) -> Mutf8Error {
    Mutf8Error { offset, kind }
}

fn decode_unit(bytes: &[u8], offset: usize) -> Result<(u16, usize), Mutf8Error> {
    let first = *bytes
        .get(offset)
        .ok_or_else(|| error(offset, Mutf8ErrorKind::UnexpectedEnd))?;
    match first {
        0 => Err(error(offset, Mutf8ErrorKind::EmbeddedNul)),
        0x01..=0x7f => Ok((first as u16, 1)),
        0xc0..=0xdf => {
            let second = *bytes
                .get(offset + 1)
                .ok_or_else(|| error(offset, Mutf8ErrorKind::UnexpectedEnd))?;
            if second & 0xc0 != 0x80 {
                return Err(error(offset + 1, Mutf8ErrorKind::InvalidContinuationByte));
            }
            let unit = (((first & 0x1f) as u16) << 6) | ((second & 0x3f) as u16);
            if unit == 0 {
                if first == 0xc0 && second == 0x80 {
                    Ok((0, 2))
                } else {
                    Err(error(offset, Mutf8ErrorKind::OverlongEncoding))
                }
            } else if unit < 0x80 {
                Err(error(offset, Mutf8ErrorKind::OverlongEncoding))
            } else {
                Ok((unit, 2))
            }
        }
        0xe0..=0xef => {
            let second = *bytes
                .get(offset + 1)
                .ok_or_else(|| error(offset, Mutf8ErrorKind::UnexpectedEnd))?;
            let third = *bytes
                .get(offset + 2)
                .ok_or_else(|| error(offset, Mutf8ErrorKind::UnexpectedEnd))?;
            if second & 0xc0 != 0x80 {
                return Err(error(offset + 1, Mutf8ErrorKind::InvalidContinuationByte));
            }
            if third & 0xc0 != 0x80 {
                return Err(error(offset + 2, Mutf8ErrorKind::InvalidContinuationByte));
            }
            let unit = (((first & 0x0f) as u16) << 12)
                | (((second & 0x3f) as u16) << 6)
                | ((third & 0x3f) as u16);
            if unit < 0x800 {
                Err(error(offset, Mutf8ErrorKind::OverlongEncoding))
            } else {
                Ok((unit, 3))
            }
        }
        _ => Err(error(offset, Mutf8ErrorKind::InvalidLeadingByte)),
    }
}

/// Validate Java Modified UTF-8 without allocating or requiring paired
/// surrogates.
pub fn validate(bytes: &[u8]) -> Result<(), Mutf8Error> {
    let mut offset = 0;
    while offset < bytes.len() {
        let (_, consumed) = decode_unit(bytes, offset)?;
        offset += consumed;
    }
    Ok(())
}

/// Decode Modified UTF-8 to its exact sequence of Java UTF-16 code units.
pub fn decode_utf16(bytes: &[u8]) -> Result<Vec<u16>, Mutf8Error> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut offset = 0;
    while offset < bytes.len() {
        let (unit, consumed) = decode_unit(bytes, offset)?;
        out.push(unit);
        offset += consumed;
    }
    Ok(out)
}

/// Decode Modified UTF-8 to a Rust string.
///
/// This rejects unpaired Java UTF-16 surrogates. Use [`decode_utf16`] when
/// exact Java-string fidelity is required for such values.
pub fn decode(bytes: &[u8]) -> Result<String, Mutf8Error> {
    let mut out = String::with_capacity(bytes.len());
    let mut offset = 0;
    while offset < bytes.len() {
        let unit_offset = offset;
        let (unit, consumed) = decode_unit(bytes, offset)?;
        offset += consumed;

        let scalar = match unit {
            0xd800..=0xdbff => {
                if offset == bytes.len() {
                    return Err(error(unit_offset, Mutf8ErrorKind::UnpairedSurrogate));
                }
                let (low, low_consumed) = decode_unit(bytes, offset)?;
                if !(0xdc00..=0xdfff).contains(&low) {
                    return Err(error(unit_offset, Mutf8ErrorKind::UnpairedSurrogate));
                }
                offset += low_consumed;
                0x1_0000 + (((unit as u32 - 0xd800) << 10) | (low as u32 - 0xdc00))
            }
            0xdc00..=0xdfff => {
                return Err(error(unit_offset, Mutf8ErrorKind::UnpairedSurrogate));
            }
            _ => unit as u32,
        };
        // All non-surrogate u16 values and correctly combined surrogate pairs
        // are valid Unicode scalar values. Keep this checked so a future
        // decoder change cannot turn malformed native input into a panic.
        let value = char::from_u32(scalar)
            .ok_or_else(|| error(unit_offset, Mutf8ErrorKind::InvalidScalarValue))?;
        out.push(value);
    }
    Ok(out)
}

/// Decode a NUL-terminated Modified UTF-8 string exactly.
pub fn decode_cstr(value: &CStr) -> Result<String, Mutf8Error> {
    decode(value.to_bytes())
}

/// Decode Modified UTF-8 while borrowing ordinary UTF-8-compatible input.
///
/// Java's special NUL and supplementary-character encodings require an owned
/// conversion. ASCII and valid one-to-three-byte UTF-8 sequences can be
/// returned without allocation after validation.
pub fn decode_cow(bytes: &[u8]) -> Result<Cow<'_, str>, Mutf8Error> {
    if bytes.iter().all(|byte| *byte != 0 && *byte < 0xf0) {
        if let Ok(value) = std::str::from_utf8(bytes) {
            return Ok(Cow::Borrowed(value));
        }
    }
    decode(bytes).map(Cow::Owned)
}

/// Decode a NUL-terminated Modified UTF-8 string, borrowing when possible.
pub fn decode_cstr_cow(value: &CStr) -> Result<Cow<'_, str>, Mutf8Error> {
    decode_cow(value.to_bytes())
}

/// Decode Modified UTF-8, replacing malformed input or unpaired surrogates.
pub fn decode_lossy(bytes: &[u8]) -> String {
    let mut utf16 = Vec::with_capacity(bytes.len());
    let mut offset = 0;
    while offset < bytes.len() {
        match decode_unit(bytes, offset) {
            Ok((unit, consumed)) => {
                utf16.push(unit);
                offset += consumed;
            }
            Err(_) => {
                utf16.push(0xfffd);
                offset += 1;
            }
        }
    }
    String::from_utf16_lossy(&utf16)
}

/// Decode a NUL-terminated Modified UTF-8 string lossily.
pub fn decode_cstr_lossy(value: &CStr) -> String {
    decode_lossy(value.to_bytes())
}
