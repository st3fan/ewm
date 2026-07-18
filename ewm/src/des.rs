//! A tiny, self-contained DES (FIPS 46-3) block cipher — just enough for the
//! VNC authentication challenge (`rfb.rs`), hand-rolled in the spirit of the
//! rest of EWM rather than pulling in a crypto crate. ECB, one 64-bit block at
//! a time, encryption only (VNC auth never decrypts).
//!
//! DES is obsolete as real cryptography and the VNC auth scheme built on it is
//! weak by design (RFC 6143 §7.2.2, notes/REMOTE.md §10) — it exists here only
//! so off-the-shelf clients (notably macOS Screen Sharing, which refuses the
//! "None" security type) will connect. Put the machine behind a tunnel or a
//! TLS-terminating proxy for anything more than a password prompt.

/// Initial permutation.
#[rustfmt::skip]
const IP: [u8; 64] = [
    58, 50, 42, 34, 26, 18, 10, 2, 60, 52, 44, 36, 28, 20, 12, 4,
    62, 54, 46, 38, 30, 22, 14, 6, 64, 56, 48, 40, 32, 24, 16, 8,
    57, 49, 41, 33, 25, 17,  9, 1, 59, 51, 43, 35, 27, 19, 11, 3,
    61, 53, 45, 37, 29, 21, 13, 5, 63, 55, 47, 39, 31, 23, 15, 7,
];

/// Final permutation (inverse of `IP`).
#[rustfmt::skip]
const FP: [u8; 64] = [
    40, 8, 48, 16, 56, 24, 64, 32, 39, 7, 47, 15, 55, 23, 63, 31,
    38, 6, 46, 14, 54, 22, 62, 30, 37, 5, 45, 13, 53, 21, 61, 29,
    36, 4, 44, 12, 52, 20, 60, 28, 35, 3, 43, 11, 51, 19, 59, 27,
    34, 2, 42, 10, 50, 18, 58, 26, 33, 1, 41,  9, 49, 17, 57, 25,
];

/// Expansion (32 → 48 bits) inside the Feistel function.
#[rustfmt::skip]
const E: [u8; 48] = [
    32,  1,  2,  3,  4,  5,  4,  5,  6,  7,  8,  9,
     8,  9, 10, 11, 12, 13, 12, 13, 14, 15, 16, 17,
    16, 17, 18, 19, 20, 21, 20, 21, 22, 23, 24, 25,
    24, 25, 26, 27, 28, 29, 28, 29, 30, 31, 32,  1,
];

/// Permutation applied to the S-box output.
#[rustfmt::skip]
const P: [u8; 32] = [
    16, 7, 20, 21, 29, 12, 28, 17, 1, 15, 23, 26, 5, 18, 31, 10,
    2, 8, 24, 14, 32, 27, 3, 9, 19, 13, 30, 6, 22, 11, 4, 25,
];

/// Permuted choice 1 (64 → 56 key bits).
#[rustfmt::skip]
const PC1: [u8; 56] = [
    57, 49, 41, 33, 25, 17,  9,  1, 58, 50, 42, 34, 26, 18,
    10,  2, 59, 51, 43, 35, 27, 19, 11,  3, 60, 52, 44, 36,
    63, 55, 47, 39, 31, 23, 15,  7, 62, 54, 46, 38, 30, 22,
    14,  6, 61, 53, 45, 37, 29, 21, 13,  5, 28, 20, 12,  4,
];

/// Permuted choice 2 (56 → 48 subkey bits).
#[rustfmt::skip]
const PC2: [u8; 48] = [
    14, 17, 11, 24,  1,  5,  3, 28, 15,  6, 21, 10,
    23, 19, 12,  4, 26,  8, 16,  7, 27, 20, 13,  2,
    41, 52, 31, 37, 47, 55, 30, 40, 51, 45, 33, 48,
    44, 49, 39, 56, 34, 53, 46, 42, 50, 36, 29, 32,
];

/// Left-rotation schedule for the two 28-bit key halves, one per round.
const SHIFTS: [u32; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

/// The eight S-boxes, each a 4×16 lookup.
#[rustfmt::skip]
const SBOX: [[u8; 64]; 8] = [
    [
        14, 4, 13, 1, 2, 15, 11, 8, 3, 10, 6, 12, 5, 9, 0, 7,
        0, 15, 7, 4, 14, 2, 13, 1, 10, 6, 12, 11, 9, 5, 3, 8,
        4, 1, 14, 8, 13, 6, 2, 11, 15, 12, 9, 7, 3, 10, 5, 0,
        15, 12, 8, 2, 4, 9, 1, 7, 5, 11, 3, 14, 10, 0, 6, 13,
    ],
    [
        15, 1, 8, 14, 6, 11, 3, 4, 9, 7, 2, 13, 12, 0, 5, 10,
        3, 13, 4, 7, 15, 2, 8, 14, 12, 0, 1, 10, 6, 9, 11, 5,
        0, 14, 7, 11, 10, 4, 13, 1, 5, 8, 12, 6, 9, 3, 2, 15,
        13, 8, 10, 1, 3, 15, 4, 2, 11, 6, 7, 12, 0, 5, 14, 9,
    ],
    [
        10, 0, 9, 14, 6, 3, 15, 5, 1, 13, 12, 7, 11, 4, 2, 8,
        13, 7, 0, 9, 3, 4, 6, 10, 2, 8, 5, 14, 12, 11, 15, 1,
        13, 6, 4, 9, 8, 15, 3, 0, 11, 1, 2, 12, 5, 10, 14, 7,
        1, 10, 13, 0, 6, 9, 8, 7, 4, 15, 14, 3, 11, 5, 2, 12,
    ],
    [
        7, 13, 14, 3, 0, 6, 9, 10, 1, 2, 8, 5, 11, 12, 4, 15,
        13, 8, 11, 5, 6, 15, 0, 3, 4, 7, 2, 12, 1, 10, 14, 9,
        10, 6, 9, 0, 12, 11, 7, 13, 15, 1, 3, 14, 5, 2, 8, 4,
        3, 15, 0, 6, 10, 1, 13, 8, 9, 4, 5, 11, 12, 7, 2, 14,
    ],
    [
        2, 12, 4, 1, 7, 10, 11, 6, 8, 5, 3, 15, 13, 0, 14, 9,
        14, 11, 2, 12, 4, 7, 13, 1, 5, 0, 15, 10, 3, 9, 8, 6,
        4, 2, 1, 11, 10, 13, 7, 8, 15, 9, 12, 5, 6, 3, 0, 14,
        11, 8, 12, 7, 1, 14, 2, 13, 6, 15, 0, 9, 10, 4, 5, 3,
    ],
    [
        12, 1, 10, 15, 9, 2, 6, 8, 0, 13, 3, 4, 14, 7, 5, 11,
        10, 15, 4, 2, 7, 12, 9, 5, 6, 1, 13, 14, 0, 11, 3, 8,
        9, 14, 15, 5, 2, 8, 12, 3, 7, 0, 4, 10, 1, 13, 11, 6,
        4, 3, 2, 12, 9, 5, 15, 10, 11, 14, 1, 7, 6, 0, 8, 13,
    ],
    [
        4, 11, 2, 14, 15, 0, 8, 13, 3, 12, 9, 7, 5, 10, 6, 1,
        13, 0, 11, 7, 4, 9, 1, 10, 14, 3, 5, 12, 2, 15, 8, 6,
        1, 4, 11, 13, 12, 3, 7, 14, 10, 15, 6, 8, 0, 5, 9, 2,
        6, 11, 13, 8, 1, 4, 10, 7, 9, 5, 0, 15, 14, 2, 3, 12,
    ],
    [
        13, 2, 8, 4, 6, 15, 11, 1, 10, 9, 3, 14, 5, 0, 12, 7,
        1, 15, 13, 8, 10, 3, 7, 4, 12, 5, 6, 11, 0, 14, 9, 2,
        7, 11, 4, 1, 9, 12, 14, 2, 0, 6, 10, 13, 15, 3, 5, 8,
        2, 1, 14, 7, 4, 10, 8, 13, 15, 12, 9, 0, 3, 5, 6, 11,
    ],
];

/// Permute the low `in_bits` of `src` according to a DES table (1-based, bit 1
/// = most significant). The result has `table.len()` bits, MSB first.
fn permute(src: u64, in_bits: u32, table: &[u8]) -> u64 {
    let mut out = 0u64;
    for &position in table {
        out = (out << 1) | ((src >> (in_bits - position as u32)) & 1);
    }
    out
}

/// Rotate the low 28 bits of `half` left by `n`.
fn rotate28(half: u32, n: u32) -> u32 {
    ((half << n) | (half >> (28 - n))) & 0x0FFF_FFFF
}

/// Derive the sixteen 48-bit round subkeys from a 64-bit key.
fn subkeys(key: u64) -> [u64; 16] {
    let permuted = permute(key, 64, &PC1); // 56 bits
    let mut c = (permuted >> 28) as u32 & 0x0FFF_FFFF;
    let mut d = permuted as u32 & 0x0FFF_FFFF;
    let mut keys = [0u64; 16];
    for (round, key) in keys.iter_mut().enumerate() {
        c = rotate28(c, SHIFTS[round]);
        d = rotate28(d, SHIFTS[round]);
        let cd = ((c as u64) << 28) | d as u64;
        *key = permute(cd, 56, &PC2);
    }
    keys
}

/// The Feistel function: expand `r`, mix the subkey, S-box substitute, permute.
fn feistel(r: u32, subkey: u64) -> u32 {
    let expanded = permute(r as u64, 32, &E) ^ subkey; // 48 bits
    let mut out = 0u32;
    for (i, sbox) in SBOX.iter().enumerate() {
        let six = (expanded >> (42 - i * 6)) & 0x3f;
        let row = ((six >> 5) & 1) << 1 | (six & 1);
        let col = (six >> 1) & 0x0f;
        out = (out << 4) | sbox[(row * 16 + col) as usize] as u32;
    }
    permute(out as u64, 32, &P) as u32
}

/// Encrypt one 64-bit block under `key` (both big-endian) with DES/ECB.
pub fn encrypt_block(key: u64, block: u64) -> u64 {
    let keys = subkeys(key);
    let permuted = permute(block, 64, &IP);
    let mut l = (permuted >> 32) as u32;
    let mut r = permuted as u32;
    for &subkey in &keys {
        let next = l ^ feistel(r, subkey);
        l = r;
        r = next;
    }
    // Final swap: the preoutput is R16 ∥ L16.
    let preoutput = ((r as u64) << 32) | l as u64;
    permute(preoutput, 64, &FP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textbook_vector() {
        // The classic worked example (key 133457799BBCDFF1,
        // plaintext 0123456789ABCDEF → 85E813540F0AB405).
        assert_eq!(
            encrypt_block(0x133457799BBCDFF1, 0x0123456789ABCDEF),
            0x85E813540F0AB405
        );
    }

    #[test]
    fn fips_all_zero_key_and_block() {
        // Known vector: key and plaintext all zero.
        assert_eq!(encrypt_block(0, 0), 0x8CA64DE9C1B123A7);
    }
}
