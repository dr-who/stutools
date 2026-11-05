//! The VPD page data the disk cases parse (SPC-6 7.7): the designation descriptors of the Device
//! Identification page (83h, table 525) and whether one names the LUN uniquely, as multipath software
//! and Windows clustering need (NAA, EUI-64 or SCSI name string of association 0; an NAA designator's
//! length follows its type), and the feature set codes of the SCSI Feature Sets page (92h, table 572).

use super::scsi_fields::be16;

/// The feature set codes in VPD page 92h: two bytes each from byte 8 (SPC-6 table 572).
pub(super) fn parse_feature_sets(d: &[u8]) -> Vec<u16> {
    let end = (4 + be16(d, 2) as usize).min(d.len());
    d.get(8..end.max(8))
        .unwrap_or(&[])
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect()
}

/// One designation descriptor of VPD page 83h (SPC-6 table 525).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Designation {
    pub code_set: u8,
    pub association: u8,
    pub kind: u8,
    pub value: Vec<u8>,
}

/// The designation descriptors, and whether they exactly fill the page length.
pub(super) fn parse_designations(d: &[u8]) -> (Vec<Designation>, bool) {
    let end = 4 + be16(d, 2) as usize;
    let mut out = Vec::new();
    let mut i = 4;
    while i + 4 <= end.min(d.len()) {
        let len = d[i + 3] as usize;
        out.push(Designation {
            code_set: d[i] & 0x0f,
            association: (d[i + 1] >> 4) & 0x3,
            kind: d[i + 1] & 0x0f,
            value: d.get(i + 4..i + 4 + len).unwrap_or(&[]).to_vec(),
        });
        i += 4 + len;
    }
    (out, i == end && end <= d.len())
}

/// Whether a designator names the LUN uniquely, as multipath software and Windows clustering need:
/// association 0, and an EUI-64 (2h), NAA (3h) of a valid NAA type, or SCSI name string (8h)
/// (SPC-6 7.7.6).
pub(super) fn names_lun(d: &Designation) -> bool {
    d.association == 0
        && match d.kind {
            2 => matches!(d.value.len(), 8 | 12 | 16),
            3 => d.code_set == 1 && naa_ok(&d.value),
            8 => matches!(d.code_set, 3) && !d.value.is_empty(),
            _ => false,
        }
}

/// An NAA designator is 8 bytes for NAA 2h, 3h and 5h and 16 bytes for NAA 6h (SPC-6 7.7.6.6).
pub(super) fn naa_ok(v: &[u8]) -> bool {
    match v.first().map(|b| b >> 4) {
        Some(2 | 3 | 5) => v.len() == 8,
        Some(6) => v.len() == 16,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_sets_parse_from_byte_8() {
        let d = [0, 0x92, 0, 8, 0, 0, 0, 0, 0x00, 0x01, 0x01, 0x02];
        assert_eq!(parse_feature_sets(&d), vec![0x0001, 0x0102]);
        assert!(parse_feature_sets(&[0, 0x92, 0, 4, 0, 0, 0, 0]).is_empty());
    }

    #[test]
    fn designations_parse_and_name_the_lun() {
        let mut d = vec![0, 0x83, 0, 0];
        d.extend_from_slice(&[0x01, 0x03, 0, 16]);
        d.extend_from_slice(&[0x60; 16]);
        d.extend_from_slice(&[0x51, 0x94, 0, 4, 0, 0, 0, 1]);
        let len = (d.len() - 4) as u16;
        d[2..4].copy_from_slice(&len.to_be_bytes());
        let (list, fills) = parse_designations(&d);
        assert!(fills);
        assert_eq!(list.len(), 2);
        assert!(names_lun(&list[0]), "NAA 6h, 16 bytes");
        assert!(
            !names_lun(&list[1]),
            "a relative target port is not the LUN"
        );
        let short = Designation {
            code_set: 1,
            association: 0,
            kind: 3,
            value: vec![0x60; 8],
        };
        assert!(!names_lun(&short), "NAA 6h must be 16 bytes");
    }
}
