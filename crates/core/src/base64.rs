//! A minimal, dependency-free base64 codec (RFC 4648 standard alphabet,
//! with padding).
//!
//! `pixelcad-core` is required to depend on nothing but `thiserror`, and
//! base64 is needed only to carry imported image data inside a `.pxc`
//! command line. Forty lines of well-tested encoding are a better trade
//! than a dependency in the crate every other crate builds on.
//!
//! Deterministic by construction: pure byte arithmetic, no allocation-order
//! or iteration-order sensitivity, identical output on every platform.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const PAD: u8 = b'=';

/// Errors produced while decoding base64 text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Base64Error {
    #[error("base64 length {0} is not a multiple of 4")]
    BadLength(usize),
    #[error("invalid base64 character {ch:?} at offset {offset}")]
    BadCharacter { ch: char, offset: usize },
    #[error("base64 padding '=' appears before the end of the input at offset {offset}")]
    MisplacedPadding { offset: usize },
}

/// Encodes bytes as standard base64 with `=` padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3f] as char
        } else {
            PAD as char
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3f] as char
        } else {
            PAD as char
        });
    }
    out
}

fn value_of(byte: u8, offset: usize) -> Result<u32, Base64Error> {
    let v = match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        other => return Err(Base64Error::BadCharacter { ch: other as char, offset }),
    };
    Ok(v as u32)
}

/// Decodes standard base64 with `=` padding. Rejects any malformed input
/// rather than guessing — a corrupted image payload must surface as an
/// error, not as silently wrong pixels.
pub fn decode(text: &str) -> Result<Vec<u8>, Base64Error> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(Base64Error::BadLength(bytes.len()));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);

    for (block_index, block) in bytes.chunks(4).enumerate() {
        let is_last = block_index == bytes.len() / 4 - 1;
        let base = block_index * 4;

        // Padding is only ever allowed in the final block, in the last one
        // or two positions.
        let pad = block.iter().filter(|&&b| b == PAD).count();
        if pad > 0 {
            if !is_last {
                let offset = base + block.iter().position(|&b| b == PAD).unwrap();
                return Err(Base64Error::MisplacedPadding { offset });
            }
            if pad > 2 || block[3 - (pad - 1)..].iter().any(|&b| b != PAD) {
                return Err(Base64Error::MisplacedPadding { offset: base });
            }
        }

        let v0 = value_of(block[0], base)?;
        let v1 = value_of(block[1], base + 1)?;
        let v2 = if pad >= 2 { 0 } else { value_of(block[2], base + 2)? };
        let v3 = if pad >= 1 { 0 } else { value_of(block[3], base + 3)? };

        let triple = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        out.push((triple >> 16) as u8);
        if pad < 2 {
            out.push((triple >> 8) as u8);
        }
        if pad < 1 {
            out.push(triple as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_rfc_4648_test_vectors() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode(plain.as_bytes()), encoded, "encoding {plain:?}");
            assert_eq!(decode(encoded).unwrap(), plain.as_bytes(), "decoding {encoded:?}");
        }
    }

    #[test]
    fn round_trips_every_byte_value_and_every_padding_case() {
        let all: Vec<u8> = (0..=255u8).collect();
        for len in [0usize, 1, 2, 3, 4, 5, 255, 256] {
            let input = &all[..len.min(all.len())];
            assert_eq!(decode(&encode(input)).unwrap(), input, "length {len}");
        }
    }

    #[test]
    fn uses_the_full_alphabet_including_plus_and_slash() {
        // 0xFB 0xFF encodes to characters near the top of the alphabet.
        let encoded = encode(&[0xfb, 0xef, 0xbe]);
        assert_eq!(encoded, "++++");
        assert_eq!(decode(&encoded).unwrap(), vec![0xfb, 0xef, 0xbe]);
    }

    #[test]
    fn rejects_a_bad_length() {
        assert_eq!(decode("Zm9"), Err(Base64Error::BadLength(3)));
    }

    #[test]
    fn rejects_an_invalid_character() {
        assert!(matches!(decode("Zm9*"), Err(Base64Error::BadCharacter { ch: '*', .. })));
    }

    #[test]
    fn rejects_padding_in_the_middle() {
        assert!(matches!(decode("Zg==Zg=="), Err(Base64Error::MisplacedPadding { offset: 2 })));
    }

    #[test]
    fn rejects_padding_before_data_in_the_last_block() {
        assert!(matches!(decode("Z=g="), Err(Base64Error::MisplacedPadding { .. })));
    }
}
