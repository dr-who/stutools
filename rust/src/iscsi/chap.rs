//! CHAP (RFC 1994, as iSCSI uses it in RFC 7143 12.1.3): the response is the digest of the chosen
//! algorithm (CHAP_A 5 MD5, 6 SHA-1, 7 SHA-256, 8 SHA3-256) over the identifier byte, the secret, then
//! the challenge. Binary values (CHAP_C, CHAP_R) travel as "0x" hex or "0b" Base64 (RFC 7143 6.1);
//! both decode, and anything else is an error naming the value. Challenges spit sends for mutual
//! CHAP are random bytes from the operating system.

use std::fmt::Write as _;

/// The CHAP algorithm numbers (CHAP_A values) this initiator implements: MD5 and SHA-1.
pub const MD5: u32 = 5;
pub const SHA1: u32 = 6;
pub const SHA256: u32 = 7;
pub const SHA3_256: u32 = 8;

/// SHA-256 digest of `input` (FIPS 180-4).
pub fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = input.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((input.len() as u64).wrapping_mul(8)).to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

const KECCAK_RC: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

/// Keccak-f[1600]: 24 rounds of theta, rho, pi, chi and iota over 25 lanes (index x + 5y).
fn keccak_f(a: &mut [u64; 25]) {
    const ROT: [[u32; 5]; 5] = [
        [0, 36, 3, 41, 18],
        [1, 44, 10, 45, 2],
        [62, 6, 43, 15, 61],
        [28, 55, 25, 21, 56],
        [27, 20, 39, 8, 14],
    ];
    for rc in KECCAK_RC {
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20];
        }
        for x in 0..5 {
            let d = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
            for y in 0..5 {
                a[x + 5 * y] ^= d;
            }
        }
        let mut b = [0u64; 25];
        for x in 0..5 {
            for y in 0..5 {
                b[y + 5 * ((2 * x + 3 * y) % 5)] = a[x + 5 * y].rotate_left(ROT[x][y]);
            }
        }
        for x in 0..5 {
            for y in 0..5 {
                a[x + 5 * y] = b[x + 5 * y] ^ (!b[(x + 1) % 5 + 5 * y] & b[(x + 2) % 5 + 5 * y]);
            }
        }
        a[0] ^= rc;
    }
}

/// SHA3-256 (FIPS 202): the Keccak sponge with a 136-byte rate and the SHA-3 padding.
pub fn sha3_256(input: &[u8]) -> [u8; 32] {
    const RATE: usize = 136;
    let mut state = [0u64; 25];
    let mut message = input.to_vec();
    message.push(0x06);
    while message.len() % RATE != 0 {
        message.push(0);
    }
    let last = message.len() - 1;
    message[last] |= 0x80;
    for block in message.chunks(RATE) {
        for (i, lane) in block.chunks(8).enumerate() {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(lane);
            state[i] ^= u64::from_le_bytes(bytes);
        }
        keccak_f(&mut state);
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = (state[i / 8] >> (8 * (i % 8))) as u8;
    }
    out
}

/// SHA-1 digest of `input` (FIPS 180-4).
pub fn sha1(input: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    let mut message = input.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((input.len() as u64).wrapping_mul(8)).to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i / 20 {
                0 => ((b & c) | (!b & d), 0x5a82_7999),
                1 => (b ^ c ^ d, 0x6ed9_eba1),
                2 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, //
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, //
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, //
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// MD5 digest of `input` (RFC 1321).
pub fn md5(input: &[u8]) -> [u8; 16] {
    let k: Vec<u32> = (0..64)
        .map(|i| ((i as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32)
        .collect();
    let mut state: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let mut message = input.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((input.len() as u64).wrapping_mul(8)).to_le_bytes());
    for block in message.chunks(64) {
        let m: Vec<u32> = block
            .chunks(4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
            .collect();
        let [mut a, mut b, mut c, mut d] = state;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

/// H(identifier || secret || challenge) for the chosen algorithm. Returns None for an algorithm this
/// initiator does not implement.
pub fn digest(algorithm: u32, identifier: u8, secret: &[u8], challenge: &[u8]) -> Option<Vec<u8>> {
    let mut input = vec![identifier];
    input.extend_from_slice(secret);
    input.extend_from_slice(challenge);
    Some(match algorithm {
        MD5 => md5(&input).to_vec(),
        SHA1 => sha1(&input).to_vec(),
        SHA256 => sha256(&input).to_vec(),
        SHA3_256 => sha3_256(&input).to_vec(),
        _ => return None,
    })
}

/// The CHAP_R response (`digest`) as "0x" and lowercase hex.
pub fn response(algorithm: u32, identifier: u8, secret: &[u8], challenge: &[u8]) -> Option<String> {
    digest(algorithm, identifier, secret, challenge).map(|d| encode_hex(&d))
}

/// A binary value as "0x" and lowercase hex (RFC 7143 6.1).
pub fn encode_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(2 + 2 * bytes.len());
    hex.push_str("0x");
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// A binary value as "0b" and Base64 with padding (RFC 7143 6.1, RFC 4648 4).
pub fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::from("0b");
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(BASE64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decodes a binary value: "0x" or "0X" and hex digits, or "0b" or "0B" and Base64 (RFC 7143 6.1). An
/// odd hex digit count is read with a leading zero, as RFC 7143 6.1 allows.
pub fn decode_binary(text: &str) -> Result<Vec<u8>, String> {
    if let Some(digits) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        let digits = if digits.len() % 2 == 1 {
            format!("0{digits}")
        } else {
            digits.to_string()
        };
        return (0..digits.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&digits[i..i + 2], 16).map_err(|_| format!("bad hex in {text}"))
            })
            .collect();
    }
    let digits = text
        .strip_prefix("0b")
        .or_else(|| text.strip_prefix("0B"))
        .ok_or_else(|| format!("CHAP value is neither hex nor Base64: {text}"))?;
    let digits = digits.trim_end_matches('=');
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for c in digits.bytes() {
        let v = BASE64
            .iter()
            .position(|&b| b == c)
            .ok_or_else(|| format!("bad Base64 in {text}"))? as u32;
        bits = bits << 6 | v;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    Ok(out)
}

/// `n` random bytes for a challenge or an identifier, from the operating system's generator; when that
/// cannot be read, from the clock mixed by xorshift, which still never repeats within a run.
pub fn random_bytes(n: usize) -> Vec<u8> {
    use std::io::Read;
    let mut out = vec![0u8; n];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut out))
        .is_ok()
    {
        return out;
    }
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        | 1;
    for byte in &mut out {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *byte = x as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: [u8; 16]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn matches_the_rfc_1321_test_suite() {
        assert_eq!(hex(md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(md5(b"a")), "0cc175b9c0f1b6a831c399e269772661");
        assert_eq!(hex(md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hex(md5(b"message digest")),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
        assert_eq!(
            hex(md5(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            )),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    #[test]
    fn handles_inputs_of_every_length_around_the_block_size() {
        // Lengths 55, 56, 63, 64 and 65 cross the padding boundaries.
        for len in [55usize, 56, 63, 64, 65, 119, 120] {
            let input = vec![b'x'; len];
            assert_eq!(md5(&input).len(), 16);
        }
    }

    #[test]
    fn sha1_matches_the_fips_180_vectors() {
        let sha1_hex = |b: &[u8]| {
            sha1(b)
                .iter()
                .map(|x| format!("{x:02x}"))
                .collect::<String>()
        };
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha1_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn sha256_and_sha3_256_match_the_standard_vectors() {
        let hex_of = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(
            hex_of(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex_of(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex_of(&sha3_256(b"")),
            "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"
        );
        assert_eq!(
            hex_of(&sha3_256(b"abc")),
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
        );
    }

    #[test]
    fn chap_response_is_the_digest_over_identifier_secret_and_challenge() {
        let mut input = vec![7u8];
        input.extend_from_slice(b"secret");
        input.extend_from_slice(&[1, 2, 3]);
        let md5_answer = response(MD5, 7, b"secret", &[1, 2, 3]).unwrap();
        assert_eq!(md5_answer, format!("0x{}", hex(md5(&input))));
        let sha1_answer = response(SHA1, 7, b"secret", &[1, 2, 3]).unwrap();
        assert_eq!(sha1_answer.len(), 2 + 40);
        assert_eq!(
            response(SHA256, 7, b"secret", &[1, 2, 3]).unwrap().len(),
            2 + 64
        );
        assert_eq!(
            response(SHA3_256, 7, b"secret", &[1, 2, 3]).unwrap().len(),
            2 + 64
        );
        assert!(response(99, 7, b"secret", &[1]).is_none());
    }

    #[test]
    fn decodes_hex_and_base64_values() {
        assert_eq!(decode_binary("0x0a1B").unwrap(), vec![0x0a, 0x1b]);
        assert_eq!(decode_binary("0Xa1b").unwrap(), vec![0x0a, 0x1b]);
        assert!(decode_binary("0a1b").is_err());
        assert!(decode_binary("0xzz").is_err());
        assert_eq!(decode_binary("0bWg==").unwrap(), vec![0x5a]);
        assert_eq!(decode_binary("0BAQID").unwrap(), vec![1, 2, 3]);
        assert!(decode_binary("0b!!").is_err());
    }

    #[test]
    fn base64_round_trips_every_length() {
        assert_eq!(encode_base64(&[0x5a]), "0bWg==");
        assert_eq!(encode_base64(b"foobar"), "0bZm9vYmFy");
        assert_eq!(encode_base64(b"fooba"), "0bZm9vYmE=");
        for len in [0usize, 1, 2, 3, 16, 1024] {
            let bytes = random_bytes(len);
            assert_eq!(decode_binary(&encode_base64(&bytes)).unwrap(), bytes);
            assert_eq!(decode_binary(&encode_hex(&bytes)).unwrap(), bytes);
        }
    }

    #[test]
    fn random_challenges_differ() {
        assert_ne!(random_bytes(16), random_bytes(16));
    }
}
