//! The persistent reservation commands the `pr` cases send and read (SPC-6 6.14, 6.15): PERSISTENT
//! RESERVE OUT with a parameter list, PERSISTENT RESERVE IN past pending unit attentions, the six
//! reservation types, REPORT CAPABILITIES (6.14.4), and the READ KEYS (6.14.2), READ RESERVATION
//! (6.14.3) and READ FULL STATUS (6.14.5) data, each parsed into what the cases compare.

use super::ops::{self, persistent_reserve_in, persistent_reserve_out, pr_parameters};
use super::scsi;
use super::session::{Outcome, Session};
use crate::Result;

/// The six reservation types (SPC-6 6.14.3.3) and their names.
pub(super) const TYPES: [(u8, &str); 6] = [
    (1, "write exclusive"),
    (3, "exclusive access"),
    (5, "write exclusive, registrants only"),
    (6, "exclusive access, registrants only"),
    (7, "write exclusive, all registrants"),
    (8, "exclusive access, all registrants"),
];

/// What REPORT CAPABILITIES says (SPC-6 6.14.4, table 179).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Capabilities {
    pub crh: bool,
    pub sip_c: bool,
    pub atp_c: bool,
    pub ptpl_c: bool,
    pub tmv: bool,
    pub allow_commands: u8,
    /// The type mask, bytes 4 and 5.
    pub mask: u16,
}

impl Capabilities {
    pub(super) fn parse(d: &[u8]) -> Self {
        let at = |i: usize| d.get(i).copied().unwrap_or(0);
        Self {
            crh: at(2) & 0x10 != 0,
            sip_c: at(2) & 0x08 != 0,
            atp_c: at(2) & 0x04 != 0,
            ptpl_c: at(2) & 0x01 != 0,
            tmv: at(3) & 0x80 != 0,
            allow_commands: (at(3) >> 4) & 0x7,
            mask: u16::from_be_bytes([at(4), at(5)]),
        }
    }

    /// Whether the device server supports reservation type `t` (table 181). Without TMV the mask says
    /// nothing, so every type is tried.
    pub(super) fn supports(&self, t: u8) -> bool {
        let bit = match t {
            1 => 0x0200,
            3 => 0x0800,
            5 => 0x2000,
            6 => 0x4000,
            7 => 0x8000,
            8 => 0x0001,
            _ => 0,
        };
        !self.tmv || self.mask & bit != 0
    }

    pub(super) fn describe(&self) -> String {
        let types: Vec<String> = TYPES
            .iter()
            .filter(|(t, _)| self.supports(*t))
            .map(|(t, _)| t.to_string())
            .collect();
        format!(
            "types {}{}, CRH {}, SIP_C {}, ATP_C {}, PTPL_C {}",
            types.join(","),
            if self.tmv { "" } else { " (no TMV)" },
            self.crh as u8,
            self.sip_c as u8,
            self.atp_c as u8,
            self.ptpl_c as u8
        )
    }
}

pub(super) fn out(
    s: &mut Session,
    action: u8,
    kind: u8,
    key: u64,
    service_key: u64,
) -> Result<Outcome> {
    persistent_reserve_out(s, action, kind, &pr_parameters(key, service_key))
}

/// READ KEYS (SPC-6 6.14.2): the generation and every registered key, once per I_T nexus.
/// PERSISTENT RESERVE IN past any unit attentions pending for the nexus, which are checked by their own
/// cases: a state query reports the state.
pub(super) fn reserve_in(s: &mut Session, action: u8, alloc: u16) -> Result<Outcome> {
    let mut o = persistent_reserve_in(s, action, alloc)?;
    for _ in 0..4 {
        if !(o.status == 2 && super::cases::sense(&o.sense).0 == 0x6) {
            break;
        }
        o = persistent_reserve_in(s, action, alloc)?;
    }
    Ok(o)
}

pub(super) fn read_keys(s: &mut Session) -> Result<(u32, Vec<u64>)> {
    let o = reserve_in(s, ops::PR_IN_READ_KEYS, 1024)?;
    scsi::check_status("PERSISTENT RESERVE IN (read keys)", o.status, &o.sense)?;
    Ok(parse_keys(&o.data))
}

pub(super) fn parse_keys(d: &[u8]) -> (u32, Vec<u64>) {
    let generation = scsi::be32(d, 0).unwrap_or(0);
    let len = scsi::be32(d, 4).unwrap_or(0) as usize;
    let keys = d
        .get(8..(8 + len).min(d.len()))
        .unwrap_or(&[])
        .chunks_exact(8)
        .map(|k| u64::from_be_bytes(k.try_into().unwrap()))
        .collect();
    (generation, keys)
}

/// READ RESERVATION (SPC-6 6.14.3): the holder's key and the type, or None when there is none.
pub(super) fn read_holder(s: &mut Session) -> Result<Option<(u64, u8)>> {
    let o = reserve_in(s, ops::PR_IN_READ_RESERVATIONS, 512)?;
    scsi::check_status(
        "PERSISTENT RESERVE IN (read reservation)",
        o.status,
        &o.sense,
    )?;
    Ok(parse_holder(&o.data))
}

pub(super) fn parse_holder(d: &[u8]) -> Option<(u64, u8)> {
    let len = scsi::be32(d, 4).unwrap_or(0);
    if len < 16 || d.len() < 22 {
        return None;
    }
    let key = u64::from_be_bytes(d[8..16].try_into().unwrap());
    Some((key, d[21] & 0x0f))
}

/// One full status descriptor (SPC-6 6.14.5, table 183).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FullStatus {
    pub(super) key: u64,
    pub(super) holder: bool,
    pub(super) all_ports: bool,
    pub(super) protocol: u8,
    /// The TransportID's name and ISID as text, e.g. `iqn...,i,0x80a153500001`.
    pub(super) transport: String,
}

pub(super) fn parse_full_status(d: &[u8]) -> Vec<FullStatus> {
    let total = scsi::be32(d, 4).unwrap_or(0) as usize;
    let end = (8 + total).min(d.len());
    let mut out = Vec::new();
    let mut i = 8;
    while i + 24 <= end {
        let len = scsi::be32(d, i + 20).unwrap_or(0) as usize;
        let id = d.get(i + 24..(i + 24 + len).min(end)).unwrap_or(&[]);
        let transport = id
            .get(4..)
            .map(|t| {
                String::from_utf8_lossy(t)
                    .trim_end_matches('\0')
                    .to_string()
            })
            .unwrap_or_default();
        out.push(FullStatus {
            key: u64::from_be_bytes(d[i..i + 8].try_into().unwrap()),
            holder: d[i + 12] & 0x01 != 0,
            all_ports: d[i + 12] & 0x02 != 0,
            protocol: id.first().copied().unwrap_or(0xff) & 0x0f,
            transport,
        });
        i += 24 + len;
    }
    out
}

pub(super) fn hex(isid: &[u8; 6]) -> String {
    isid.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn show_holder(h: &Result<Option<(u64, u8)>>) -> String {
    match h {
        Ok(Some((k, t))) => format!("key {k:#x}, type {t}"),
        Ok(None) => "none".into(),
        Err(err) => err.clone(),
    }
}

pub(super) fn show_keys(k: &Result<(u32, Vec<u64>)>) -> String {
    match k {
        Ok((_, keys)) if keys.is_empty() => "none".into(),
        Ok((_, keys)) => {
            let all: Vec<String> = keys.iter().map(|k| format!("{k:#x}")).collect();
            all.join(", ")
        }
        Err(err) => err.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::pr_ports::{ISID_A, KEY_A, KEY_B};
    use super::*;

    #[test]
    fn capabilities_decode_the_type_mask() {
        // CRH and PTPL_C; TMV; WR_EX, EX_AC, WR_EX_RO, EX_AC_RO, WR_EX_AR and EX_AC_AR.
        let caps = Capabilities::parse(&[0, 8, 0x11, 0x80, 0xea, 0x01, 0, 0]);
        assert!(caps.crh && caps.ptpl_c && caps.tmv && !caps.sip_c);
        for t in [1, 3, 5, 6, 7, 8] {
            assert!(caps.supports(t), "type {t}");
        }
        let only_we = Capabilities::parse(&[0, 8, 0, 0x80, 0x02, 0, 0, 0]);
        assert!(only_we.supports(1) && !only_we.supports(5) && !only_we.supports(8));
        let no_tmv = Capabilities::parse(&[0, 8, 0, 0, 0, 0, 0, 0]);
        assert!(no_tmv.supports(5));
    }

    #[test]
    fn keys_and_holder_parse() {
        let mut d = vec![0, 0, 0, 7, 0, 0, 0, 16];
        d.extend_from_slice(&KEY_A.to_be_bytes());
        d.extend_from_slice(&KEY_B.to_be_bytes());
        assert_eq!(parse_keys(&d), (7, vec![KEY_A, KEY_B]));
        let mut r = vec![0, 0, 0, 1, 0, 0, 0, 16];
        r.extend_from_slice(&KEY_A.to_be_bytes());
        r.extend_from_slice(&[0, 0, 0, 0, 0, 0x05, 0, 0]);
        assert_eq!(parse_holder(&r), Some((KEY_A, 5)));
        assert_eq!(parse_holder(&[0, 0, 0, 1, 0, 0, 0, 0]), None);
    }

    #[test]
    fn full_status_reads_holder_and_transport_id() {
        let name = b"iqn.x,i,0x80a153500001\0\0";
        let mut id = vec![0x45, 0, 0, name.len() as u8];
        id.extend_from_slice(name);
        let mut desc = KEY_A.to_be_bytes().to_vec();
        desc.extend_from_slice(&[0, 0, 0, 0, 0x01, 0x01, 0, 0, 0, 0, 0, 1]);
        desc.extend_from_slice(&(id.len() as u32).to_be_bytes());
        desc.extend_from_slice(&id);
        let mut d = vec![0, 0, 0, 3];
        d.extend_from_slice(&(desc.len() as u32).to_be_bytes());
        d.extend_from_slice(&desc);
        let list = parse_full_status(&d);
        assert_eq!(list.len(), 1);
        assert!(list[0].holder && !list[0].all_ports);
        assert_eq!(list[0].protocol, 5);
        assert_eq!(list[0].transport, "iqn.x,i,0x80a153500001");
        assert_eq!(hex(&ISID_A), "80a153500001");
    }
}
