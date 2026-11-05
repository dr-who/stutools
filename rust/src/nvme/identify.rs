//! The Identify data structures (Base 2.1 5.1.13; NVM 1.1 Identify Namespace) and their parsers:
//! the CNS values, Identify Controller (Figure 312) and Identify Namespace fields spit reports or
//! tests, the active namespace list (CNS 02h), and the namespace identification descriptor list
//! (CNS 03h, Figure 315) with the rule that its EUI-64 and NGUID copy Identify Namespace. Each parser
//! reads the 4096-byte data structure Identify returns.

/// Identify CNS values.
pub const CNS_NAMESPACE: u32 = 0x00;
pub const CNS_CONTROLLER: u32 = 0x01;
pub const CNS_ACTIVE_NAMESPACES: u32 = 0x02;
pub const CNS_NS_DESCRIPTORS: u32 = 0x03;

/// Namespace Identifier Types and the length each has (Base 2.1 Figure 315).
pub const NIDT_EUI64: u8 = 1;
pub const NIDT_NGUID: u8 = 2;
pub const NIDT_UUID: u8 = 3;
pub const NIDT_CSI: u8 = 4;

/// The fields of Identify Controller that spit reports or tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerInfo {
    /// Bytes of write data the controller takes in the capsule (IOCCSZ less the 64-byte command).
    /// The controller expects in-capsule data for writes up to this size, so the host must send it.
    pub inline_bytes: usize,
    pub serial: String,
    pub model: String,
    pub firmware: String,
    pub namespaces: u32,
    /// Maximum data transfer size as a power of two in units of the minimum page size, 0 for none.
    pub mdts: u8,
    pub nqn: String,
    /// CMIC: bit 1 more than one controller, bit 3 ANA reporting.
    pub cmic: u8,
    pub cntlid: u16,
    /// VER: the version in the same form as the VS property.
    pub ver: u32,
    /// AERL: the asynchronous event requests outstanding at once, 0-based.
    pub aerl: u8,
    /// KAS: the keep-alive timer granularity in 100 ms units.
    pub kas: u16,
    /// ANACAP, ANAGRPMAX and NANAGRPID: the ANA states and groups the controller can report.
    pub anacap: u8,
    pub anagrpmax: u32,
    pub nanagrpid: u32,
    /// SQES and CQES: required (low nibble) and largest (high nibble) entry sizes as powers of two.
    pub sqes: u8,
    pub cqes: u8,
    /// ONCS: optional NVM commands (bit 0 Compare, 2 Dataset Management, 3 Write Zeroes,
    /// 5 Reservations).
    pub oncs: u16,
    /// FUSES: bit 0 fused Compare and Write.
    pub fuses: u16,
    /// VWC: bit 0 a volatile write cache is present.
    pub vwc: u8,
    /// SGLS: SGL support; bits 1:0 are non-zero when SGLs are supported, which fabrics need.
    pub sgls: u32,
    /// IOCCSZ and IORCSZ: the I/O command and response capsule sizes in 16-byte units.
    pub ioccsz: u32,
    pub iorcsz: u32,
}

pub fn parse_controller(d: &[u8]) -> ControllerInfo {
    let u16_at = |at: usize| u16::from_le_bytes([d[at], d[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]);
    let ioccsz = u32_at(1792);
    ControllerInfo {
        inline_bytes: (ioccsz as usize * 16).saturating_sub(64),
        serial: ascii(&d[4..24]),
        model: ascii(&d[24..64]),
        firmware: ascii(&d[64..72]),
        namespaces: u32_at(516),
        mdts: d[77],
        nqn: ascii(&d[768..1024]),
        cmic: d[76],
        cntlid: u16_at(78),
        ver: u32_at(80),
        aerl: d[259],
        kas: u16_at(320),
        anacap: d[343],
        anagrpmax: u32_at(344),
        nanagrpid: u32_at(348),
        sqes: d[512],
        cqes: d[513],
        oncs: u16_at(520),
        fuses: u16_at(522),
        vwc: d[525],
        sgls: u32_at(536),
        ioccsz,
        iorcsz: u32_at(1796),
    }
}

/// The fields of Identify Namespace that spit reports or tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamespaceInfo {
    /// Namespace size in logical blocks (NSZE).
    pub blocks: u64,
    pub block_len: u32,
    /// NCAP and NUSE: the capacity and the blocks in use.
    pub capacity: u64,
    pub used: u64,
    /// NLBAF (0-based) and the format index FLBAS selects.
    pub formats: u8,
    pub format: u8,
    /// NMIC bit 0: the namespace may be attached to more than one controller.
    pub nmic: u8,
    /// RESCAP: bit 1 to 6 the reservation types 1 to 6, bit 7 Ignore Existing Key.
    pub rescap: u8,
    /// DLFEAT bits 2:0: what a deallocated block reads as (1 zeros, 2 ones, 0 not reported).
    pub dlfeat: u8,
    pub anagrpid: u32,
    pub nguid: [u8; 16],
    pub eui64: [u8; 8],
}

pub fn parse_namespace(d: &[u8]) -> NamespaceInfo {
    let u64_at = |at: usize| u64::from_le_bytes(d[at..at + 8].try_into().unwrap_or([0; 8]));
    let blocks = u64_at(0);
    // NVM 1.1 Identify Namespace FLBAS: the format index is bits 3:0, with bits 6:5 above them when
    // there are more than 16 formats (NLBAF is 0-based).
    let flbas = if d[25] >= 16 {
        (d[26] & 0x0f) as usize | ((d[26] as usize >> 5) & 0x3) << 4
    } else {
        (d[26] & 0x0f) as usize
    };
    let lbaf = u32::from_le_bytes(
        d[128 + 4 * flbas..132 + 4 * flbas]
            .try_into()
            .unwrap_or([0; 4]),
    );
    let lbads = (lbaf >> 16) & 0xff;
    NamespaceInfo {
        blocks,
        block_len: 1u32 << lbads.min(31),
        capacity: u64_at(8),
        used: u64_at(16),
        formats: d[25],
        format: flbas as u8,
        nmic: d[30],
        rescap: d[31],
        dlfeat: d[33],
        anagrpid: u32::from_le_bytes([d[92], d[93], d[94], d[95]]),
        nguid: d[104..120].try_into().unwrap_or([0; 16]),
        eui64: d[120..128].try_into().unwrap_or([0; 8]),
    }
}

/// The active namespace IDs listed by CNS 2, in order, until the zero terminator.
pub fn parse_active_namespaces(d: &[u8]) -> Vec<u32> {
    d.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .take_while(|&nsid| nsid != 0)
        .collect()
}

fn ascii(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .trim_end_matches([' ', '\0'])
        .trim()
        .to_string()
}

/// One identification descriptor: its type and value.
pub type Descriptor = (u8, Vec<u8>);

/// Parses a CNS 03h list (Base 2.1 5.1.13.2.3, Figure 315): 4-byte headers (type, length, two reserved
/// bytes) and values, ending at a zero length. A defined type must have its defined length, and no
/// type may appear twice.
pub fn parse_descriptors(d: &[u8]) -> std::result::Result<Vec<Descriptor>, String> {
    let mut out: Vec<Descriptor> = Vec::new();
    let mut i = 0;
    while i + 4 <= d.len() && d[i + 1] != 0 {
        let (kind, len) = (d[i], d[i + 1] as usize);
        let want = match kind {
            NIDT_EUI64 => Some(8),
            NIDT_NGUID | NIDT_UUID => Some(16),
            NIDT_CSI => Some(1),
            _ => None,
        };
        if want.is_some_and(|w| w != len) {
            return Err(format!("type {kind} has length {len}"));
        }
        if out.iter().any(|(k, _)| *k == kind) {
            return Err(format!("type {kind} appears twice"));
        }
        let value = d
            .get(i + 4..i + 4 + len)
            .ok_or_else(|| format!("type {kind} runs past the list"))?;
        out.push((kind, value.to_vec()));
        i += 4 + len;
    }
    Ok(out)
}

/// The unique IDs among the descriptors (EUI-64, NGUID, UUID) that are not zero, sorted.
pub fn unique_ids(list: &[Descriptor]) -> Vec<Descriptor> {
    let mut ids: Vec<Descriptor> = list
        .iter()
        .filter(|(k, v)| {
            matches!(*k, NIDT_EUI64 | NIDT_NGUID | NIDT_UUID) && v.iter().any(|&b| b != 0)
        })
        .cloned()
        .collect();
    ids.sort();
    ids
}

/// Base 2.1 Figure 315: an EUI-64 or NGUID descriptor is a copy of the Identify Namespace field, and
/// is reported only when that field is not zero; a set field must therefore be in the list.
pub fn agrees(ns: &NamespaceInfo, list: &[Descriptor]) -> bool {
    let same = |kind: u8, value: &[u8]| {
        let zero = value.iter().all(|&b| b == 0);
        match list.iter().find(|(k, _)| *k == kind) {
            Some((_, v)) => !zero && v == value,
            None => zero,
        }
    };
    same(NIDT_NGUID, &ns.nguid) && same(NIDT_EUI64, &ns.eui64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_controller_fields_come_from_their_offsets() {
        let mut d = vec![0u8; 4096];
        d[76] = 0x0b;
        d[78..80].copy_from_slice(&7u16.to_le_bytes());
        d[80..84].copy_from_slice(&0x0002_0000u32.to_le_bytes());
        d[320..322].copy_from_slice(&5u16.to_le_bytes());
        d[512] = 0x66;
        d[520..522].copy_from_slice(&0x2cu16.to_le_bytes());
        d[525] = 1;
        d[536..540].copy_from_slice(&1u32.to_le_bytes());
        d[1792..1796].copy_from_slice(&260u32.to_le_bytes());
        let c = parse_controller(&d);
        assert_eq!((c.cmic, c.cntlid, c.ver, c.kas), (0x0b, 7, 0x0002_0000, 5));
        assert_eq!((c.sqes, c.oncs, c.vwc, c.sgls), (0x66, 0x2c, 1, 1));
        assert_eq!(c.inline_bytes, 260 * 16 - 64);
    }

    #[test]
    fn the_format_index_uses_its_upper_bits_past_16_formats() {
        let mut d = vec![0u8; 4096];
        d[25] = 17; // 18 formats
        d[26] = 0x21; // index 0b01_0001 = 17
        d[128 + 4 * 17 + 2] = 12; // LBADS 12: 4096-byte blocks
        let ns = parse_namespace(&d);
        assert_eq!((ns.format, ns.block_len), (17, 4096));
        d[25] = 3; // 4 formats: bits 6:5 are ignored
        d[128 + 4 + 2] = 9;
        assert_eq!(parse_namespace(&d).block_len, 512);
    }

    fn descriptor(kind: u8, value: &[u8]) -> Vec<u8> {
        let mut d = vec![kind, value.len() as u8, 0, 0];
        d.extend_from_slice(value);
        d
    }

    #[test]
    fn descriptors_parse_until_a_zero_length() {
        let mut d = descriptor(NIDT_UUID, &[7; 16]);
        d.extend(descriptor(NIDT_CSI, &[0]));
        d.resize(4096, 0);
        let list = parse_descriptors(&d).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(unique_ids(&list), vec![(NIDT_UUID, vec![7; 16])]);
    }

    #[test]
    fn a_wrong_length_a_repeat_or_a_zero_id_is_caught() {
        let mut d = descriptor(NIDT_NGUID, &[1; 8]);
        d.resize(64, 0);
        assert!(parse_descriptors(&d).is_err());
        let mut d = descriptor(NIDT_UUID, &[1; 16]);
        d.extend(descriptor(NIDT_UUID, &[2; 16]));
        d.resize(64, 0);
        assert!(parse_descriptors(&d).is_err());
        let mut d = descriptor(NIDT_EUI64, &[0; 8]);
        d.resize(64, 0);
        assert!(unique_ids(&parse_descriptors(&d).unwrap()).is_empty());
    }

    #[test]
    fn identify_namespace_ids_must_match_the_descriptors() {
        let mut d = vec![0u8; 4096];
        d[104..120].copy_from_slice(&[9; 16]);
        let ns = parse_namespace(&d);
        assert!(agrees(&ns, &[(NIDT_NGUID, vec![9; 16])]));
        assert!(!agrees(&ns, &[(NIDT_UUID, vec![9; 16])]));
        // An EUI-64 descriptor while Identify Namespace reports none.
        assert!(!agrees(
            &ns,
            &[(NIDT_NGUID, vec![9; 16]), (NIDT_EUI64, vec![1; 8])]
        ));
    }
}
