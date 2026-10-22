// A seeded 4 KiB pattern with an absolute address in every 512-byte sector.
// Overlapping writes agree byte-for-byte. Pure write workers can reuse the
// seeded payload and only update addresses, like the C engine's reusable data.
fn word(offset: u64, seed: u64) -> [u8; 8] {
    if offset % 512 == 0 {
        return (offset ^ seed.wrapping_mul(0x9e3779b97f4a7c15)).to_le_bytes();
    }
    mix(((offset % 4096) / 8) ^ seed.wrapping_mul(0x9e3779b97f4a7c15)).to_le_bytes()
}
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
pub fn fill(mut buf: &mut [u8], mut offset: u64, seed: u64) {
    while !buf.is_empty() {
        let skip = (offset % 8) as usize;
        let n = (8 - skip).min(buf.len());
        buf[..n].copy_from_slice(&word(offset & !7, seed)[skip..skip + n]);
        buf = &mut buf[n..];
        offset += n as u64;
    }
}
pub fn stamp(buf: &mut [u8], offset: u64, seed: u64) {
    debug_assert_eq!(offset % 512, 0);
    for (i, sector) in buf.chunks_mut(512).enumerate() {
        let n = sector.len().min(8);
        sector[..n].copy_from_slice(&word(offset + i as u64 * 512, seed)[..n]);
    }
}
pub fn fill_cached(buf: &mut [u8], offset: u64, seed: u64, template: &[u8; 4096]) {
    if offset % 512 != 0 || buf.len() % 512 != 0 {
        fill(buf, offset, seed);
        return;
    }
    let mut remaining = &mut buf[..];
    let mut phase = offset as usize % template.len();
    while !remaining.is_empty() {
        let n = remaining.len().min(template.len() - phase);
        remaining[..n].copy_from_slice(&template[phase..phase + n]);
        remaining = &mut remaining[n..];
        phase = 0;
    }
    stamp(buf, offset, seed);
}
pub fn matches_cached(buf: &[u8], offset: u64, seed: u64, template: &[u8; 4096]) -> bool {
    if offset % 512 != 0 || buf.len() % 512 != 0 {
        return matches(buf, offset, seed);
    }
    for (i, sector) in buf.chunks_exact(512).enumerate() {
        let address = offset + i as u64 * 512;
        let phase = address as usize % template.len();
        if sector[..8] != word(address, seed) || sector[8..] != template[phase + 8..phase + 512] {
            return false;
        }
    }
    true
}
pub fn matches(mut buf: &[u8], mut offset: u64, seed: u64) -> bool {
    while !buf.is_empty() {
        let skip = (offset % 8) as usize;
        let n = (8 - skip).min(buf.len());
        if buf[..n] != word(offset & !7, seed)[skip..skip + n] {
            return false;
        }
        buf = &buf[n..];
        offset += n as u64;
    }
    true
}
pub fn random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    mix(*state)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_writes_agree() {
        let mut full = [0; 8192];
        fill(&mut full, 0, 7);
        let mut partial = [0; 5011];
        fill(&mut partial, 13, 7);
        assert_eq!(partial, &full[13..5024]);
        assert!(matches(&partial, 13, 7));
        partial[3] ^= 1;
        assert!(!matches(&partial, 13, 7));
    }
    #[test]
    fn reused_pattern_matches() {
        let mut data = [0; 8192];
        fill(&mut data, 0, 9);
        stamp(&mut data, 32768, 9);
        assert!(matches(&data, 32768, 9));
        assert!(!matches(&data, 0, 9));
    }
    #[test]
    fn cached_pattern_preserves_bytes_and_detects_corruption() {
        for seed in [0, 1, u64::MAX] {
            let mut template = [0; 4096];
            fill(&mut template, 0, seed);
            for offset in [0, 13, 512, 1536, 4096, 65536] {
                for len in [1, 511, 512, 1024, 4096, 8192] {
                    let mut expected = [0; 8192];
                    let mut actual = [0xaa; 8192];
                    fill(&mut expected[..len], offset, seed);
                    fill_cached(&mut actual[..len], offset, seed, &template);
                    assert_eq!(actual[..len], expected[..len]);
                    assert!(matches_cached(&actual[..len], offset, seed, &template));
                    for index in [0, len / 2, len - 1] {
                        actual[index] ^= 1;
                        assert!(!matches_cached(&actual[..len], offset, seed, &template));
                        actual[index] ^= 1;
                    }
                }
            }
        }
    }
}
