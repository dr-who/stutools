//! The ANA log page (Base 2.1 5.1.12.1.13, Figures 223 and 224): one descriptor per Asymmetric
//! Namespace Access group with its ID, state and namespace IDs, the names of the states, and the
//! ANACAP bit that allows each state. Parsing fails on a descriptor or namespace list that runs past
//! the page.

/// Log page identifier of the ANA log page.
pub const LOG_ANA: u8 = 0x0c;
/// The log page buffer: a header, and per group a descriptor and its namespace IDs.
pub const LOG_LEN: usize = 4096;

/// One ANA group descriptor: its ID, state and namespaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: u32,
    pub state: u8,
    pub nsids: Vec<u32>,
}

/// Parses the ANA log page: a 16-byte header with the group count at bytes 8-9, then per group a
/// 32-byte descriptor (ANAGRPID, NNSIDS, change count, ANA state in byte 16 bits 3:0) and NNSIDS
/// namespace IDs of 4 bytes.
pub fn parse(d: &[u8]) -> std::result::Result<Vec<Group>, String> {
    let groups = u16::from_le_bytes([d[8], d[9]]) as usize;
    let mut out = Vec::new();
    let mut i = 16;
    for g in 0..groups {
        let desc = d
            .get(i..i + 32)
            .ok_or_else(|| format!("descriptor {g} runs past the page"))?;
        let id = u32::from_le_bytes(desc[0..4].try_into().unwrap());
        let n = u32::from_le_bytes(desc[4..8].try_into().unwrap()) as usize;
        let list = d
            .get(i + 32..i + 32 + 4 * n)
            .ok_or_else(|| format!("group {id} lists {n} namespaces past the page"))?;
        out.push(Group {
            id,
            state: desc[16] & 0x0f,
            nsids: list
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        });
        i += 32 + 4 * n;
    }
    Ok(out)
}

/// The name of an ANA state.
pub fn state_name(state: u8) -> &'static str {
    match state {
        0x1 => "optimized",
        0x2 => "non-optimized",
        0x3 => "inaccessible",
        0x4 => "persistent loss",
        0xf => "change",
        _ => "reserved",
    }
}

/// The ANACAP bit that says the controller may report `state`.
pub fn anacap_bit(state: u8) -> Option<u8> {
    match state {
        0x1 => Some(0x01),
        0x2 => Some(0x02),
        0x3 => Some(0x04),
        0x4 => Some(0x08),
        0xf => Some(0x10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_page_lists_groups_and_their_namespaces() {
        let mut d = vec![0u8; 256];
        d[8] = 2;
        // group 1: optimized, namespaces 1 and 2
        d[16..20].copy_from_slice(&1u32.to_le_bytes());
        d[20..24].copy_from_slice(&2u32.to_le_bytes());
        d[32] = 0x1;
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[52..56].copy_from_slice(&2u32.to_le_bytes());
        // group 2: inaccessible, no namespaces
        d[56..60].copy_from_slice(&2u32.to_le_bytes());
        d[72] = 0x3;
        let g = parse(&d).unwrap();
        assert_eq!(g.len(), 2);
        assert_eq!(
            g[0],
            Group {
                id: 1,
                state: 1,
                nsids: vec![1, 2]
            }
        );
        assert_eq!(g[1].state, 3);
        assert_eq!(anacap_bit(3), Some(0x04));
    }

    #[test]
    fn a_group_past_the_page_is_an_error() {
        let mut d = vec![0u8; 40];
        d[8] = 1;
        d[20..24].copy_from_slice(&100u32.to_le_bytes());
        assert!(parse(&d).is_err());
    }
}
