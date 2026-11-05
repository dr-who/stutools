//! The nvme-ident cases of `--compliance` (Base 2.1 5.1.13 Identify; NVM 1.1 Identify Namespace): the
//! Identify Controller fields a host depends on, each active namespace's Identify Namespace data and
//! its identification descriptors (CNS 03h), and the same identity on every other path. Windows MPIO
//! groups paths by the namespace's unique ID (EUI-64, NGUID or UUID) and the serial number, model and
//! subsystem NQN, so every controller of the subsystem must report the same ones.
//! Each case names the NVMe revision that defines what it tests and is judged against the revision
//! the controller reports. Returns what it learned for the later sections, or None when Identify
//! itself fails.

use std::net::SocketAddr;

use super::compliance::{rev_case, Target, SUCCESS};
use super::controller::{self, Association, Controller, ControllerInfo, NamespaceInfo};
use super::revisions::{self as rev, Rev};
use super::DEFAULT_HOSTNQN;
use crate::compliance::{case, Case};
use crate::Result;

/// Namespace Identifier Types and the length each has (Base 2.1 Figure 315).
const NIDT_EUI64: u8 = 1;
const NIDT_NGUID: u8 = 2;
const NIDT_UUID: u8 = 3;
const NIDT_CSI: u8 = 4;

/// CMIC bit 1 MCTRS: the NVM subsystem may have two or more controllers (Base 2.1 Figure 312).
const CMIC_MULTI_CONTROLLER: u8 = 0x02;
/// NMIC bit 0: the namespace may be attached to two or more controllers (NVM 1.1 Identify Namespace).
const NMIC_SHARED: u8 = 0x01;
/// RESCAP bit 7 IEKS: Ignore Existing Key behaves as revision 1.3 and later define it.
const RESCAP_IEKS: u8 = 0x80;

/// What the identify cases learned about a subsystem.
pub struct Facts {
    pub ctrl: ControllerInfo,
    /// The CAP property, read before enabling.
    pub cap: u64,
    /// The revision the controller implements: the VS property, or VER when VS could not be read.
    pub vs: u32,
    pub active: Vec<u32>,
    /// The active namespaces with sane geometry, which the I/O sections use.
    pub namespaces: Vec<(u32, NamespaceInfo)>,
    /// Every active namespace has a non-zero unique ID, and every path reports the same ones.
    pub unique_ids: bool,
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

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn show_ids(ids: &[Descriptor]) -> String {
    if ids.is_empty() {
        return "none".into();
    }
    ids.iter()
        .map(|(k, v)| {
            let kind = match *k {
                NIDT_EUI64 => "EUI-64",
                NIDT_NGUID => "NGUID",
                _ => "UUID",
            };
            format!("{kind} {}", hex(v))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Base 2.1 Figure 315: an EUI-64 or NGUID descriptor is a copy of the Identify Namespace field, and
/// is reported only when that field is not zero; a set field must therefore be in the list.
fn agrees(ns: &NamespaceInfo, list: &[Descriptor]) -> bool {
    let same = |kind: u8, value: &[u8]| {
        let zero = value.iter().all(|&b| b == 0);
        match list.iter().find(|(k, _)| *k == kind) {
            Some((_, v)) => !zero && v == value,
            None => zero,
        }
    };
    same(NIDT_NGUID, &ns.nguid) && same(NIDT_EUI64, &ns.eui64)
}

/// The identity one controller reports: serial, model, subsystem NQN, and each namespace's IDs.
#[derive(Debug, PartialEq)]
struct PathIdentity {
    serial: String,
    model: String,
    nqn: String,
    namespaces: Vec<(u32, Vec<Descriptor>)>,
}

fn path_identity(c: &mut Controller) -> Result<PathIdentity> {
    let info = controller::parse_controller(&c.identify(controller::CNS_CONTROLLER, 0)?);
    let active =
        controller::parse_active_namespaces(&c.identify(controller::CNS_ACTIVE_NAMESPACES, 0)?);
    let mut namespaces = Vec::new();
    for nsid in active {
        let list = parse_descriptors(&c.identify(controller::CNS_NS_DESCRIPTORS, nsid)?)?;
        namespaces.push((nsid, unique_ids(&list)));
    }
    Ok(PathIdentity {
        serial: info.serial,
        model: info.model,
        nqn: info.nqn,
        namespaces,
    })
}

fn connect_path(addr: SocketAddr, subnqn: &str) -> Result<Controller> {
    let assoc = Association {
        addr,
        subnqn,
        hostnqn: DEFAULT_HOSTNQN,
        hostid: controller::random_hostid()?,
    };
    let mut c = Controller::connect(&assoc)?;
    c.enable()?;
    Ok(c)
}

fn ver(v: u32) -> String {
    rev::show(rev::version(v))
}

/// Runs the nvme-ident cases on the enabled controller `c`, then compares the identity of a second
/// association and of every other address of the subsystem.
pub fn run(
    c: &mut Controller,
    name: &str,
    t: &Target,
    cap: u64,
    vs: Option<u64>,
    cases: &mut Vec<Case>,
) -> Option<Facts> {
    let ident = |cases: &mut Vec<Case>, n: String, expected: &str, actual: String, passed: bool| {
        cases.push(case("nvme-ident", n, expected.into(), actual, passed));
    };
    let named = |r: &Rev, what: &str| format!("{name} {}{what}", r.name());
    let info = match c.identify(controller::CNS_CONTROLLER, 0) {
        Ok(d) => controller::parse_controller(&d),
        Err(e) => {
            ident(
                cases,
                named(&rev::IDENTIFY_CONTROLLER, ""),
                "identified",
                e,
                false,
            );
            return None;
        }
    };
    let vs = vs.map_or(info.ver, |v| v as u32);
    // Base 2.1 Figure 312: SUBNQN is mandatory from 1.2.1; SN and MN are mandatory.
    let subnqn_due = rev::SUBNQN_FIELD.defined_in(vs);
    ident(
        cases,
        named(&rev::IDENTIFY_CONTROLLER, ""),
        &format!(
            "SUBNQN {}{}, a serial and model",
            t.subnqn,
            if subnqn_due {
                ""
            } else {
                " (or empty before 1.2.1)"
            }
        ),
        format!(
            "SUBNQN {}, serial {:?}, model {:?}",
            info.nqn, info.serial, info.model
        ),
        (info.nqn == t.subnqn || (!subnqn_due && info.nqn.is_empty()))
            && !info.serial.is_empty()
            && !info.model.is_empty(),
    );
    // Base 2.1 Figure 312: CNTLID is the controller's ID, which Connect returned.
    ident(
        cases,
        format!("{name} NVMe 1.1 CNTLID"),
        &format!("{:#06x}, the ID Connect returned", c.cntlid()),
        format!("{:#06x}", info.cntlid),
        info.cntlid == c.cntlid(),
    );
    // Base 2.1 Figure 312: VER holds the VS property's value, non-zero from revision 1.2.
    let ver_due = rev::VER_FIELD.defined_in(vs);
    ident(
        cases,
        named(&rev::VER_FIELD, ""),
        &format!(
            "VS {}{}",
            ver(vs),
            if ver_due { "" } else { ", or zero before 1.2" }
        ),
        ver(info.ver),
        info.ver == vs || (!ver_due && info.ver == 0),
    );
    // Base 2.1 Figure 312: the required SQ and CQ entry sizes shall be 6 (64 bytes) and 4 (16 bytes).
    ident(
        cases,
        format!("{name} NVMe 1.0 SQES and CQES"),
        "required 64-byte submission and 16-byte completion entries",
        format!("SQES {:#04x}, CQES {:#04x}", info.sqes, info.cqes),
        info.sqes & 0xf == 6 && info.sqes >> 4 >= 6 && info.cqes & 0xf == 4 && info.cqes >> 4 >= 4,
    );
    // Base 2.1 Figure 312: SGLS is mandatory for fabrics (Disc M), bits 1:0 non-zero when supported;
    // NVMe over Fabrics commands carry SGLs only.
    ident(
        cases,
        format!("{name} NVMe 1.1 SGLS"),
        "SGLs supported",
        format!("{:#010x}", info.sgls),
        info.sgls & 0x3 != 0,
    );
    // Base 2.1 Figure 312: IOCCSZ is at least 4 (the 64-byte command), IORCSZ at least 1 (the 16-byte
    // completion), in 16-byte units.
    ident(
        cases,
        format!("{name} NVMe 1.2.1 IOCCSZ and IORCSZ"),
        "4 or more, 1 or more (16-byte units)",
        format!("{}, {}", info.ioccsz, info.iorcsz),
        info.ioccsz >= 4 && info.iorcsz >= 1,
    );
    ident(
        cases,
        format!("{name} optional features"),
        "reported",
        format!(
            "VS {}, CMIC {:#04x}, ONCS {:#06x}, FUSES {:#06x}, VWC {:#04x}, MDTS {}, AERL {}",
            ver(vs),
            info.cmic,
            info.oncs,
            info.fuses,
            info.vwc,
            info.mdts,
            info.aerl
        ),
        true,
    );

    // Base 2.1 5.1.13.2.2: active NSIDs in increasing order; Figure 312 NN bounds them.
    let active = match c.identify(controller::CNS_ACTIVE_NAMESPACES, 0) {
        Ok(d) => controller::parse_active_namespaces(&d),
        Err(e) => {
            ident(
                cases,
                named(&rev::ACTIVE_NAMESPACES, ""),
                "listed",
                e,
                false,
            );
            return None;
        }
    };
    ident(
        cases,
        named(&rev::ACTIVE_NAMESPACES, ""),
        "listed, ascending, none above NN",
        format!("{active:?}, NN {}", info.namespaces),
        active.windows(2).all(|w| w[0] < w[1]) && active.iter().all(|&n| n <= info.namespaces),
    );

    let mut namespaces = Vec::new();
    let mut unique = !active.is_empty();
    let shared_expected = info.cmic & CMIC_MULTI_CONTROLLER != 0 || t.addrs.len() > 1;
    for &nsid in &active {
        let ns = match c.identify(controller::CNS_NAMESPACE, nsid) {
            Ok(d) => controller::parse_namespace(&d),
            Err(e) => {
                ident(
                    cases,
                    named(&rev::IDENTIFY_NAMESPACE, &format!(" {nsid}")),
                    "identified",
                    e,
                    false,
                );
                unique = false;
                continue;
            }
        };
        let geometry = ns.blocks > 0 && ns.block_len >= 512 && ns.block_len.is_power_of_two();
        ident(
            cases,
            named(&rev::IDENTIFY_NAMESPACE, &format!(" {nsid}")),
            "blocks > 0 of 512 B or more, a power of two",
            format!("{} blocks of {} B", ns.blocks, ns.block_len),
            geometry,
        );
        // NVM 1.1 Identify Namespace: NCAP never exceeds NSZE, NUSE never exceeds NCAP.
        ident(
            cases,
            format!("{name} ns {nsid} NVMe 1.0 NSZE, NCAP, NUSE"),
            "NSZE >= NCAP >= NUSE",
            format!("{} >= {} >= {}", ns.blocks, ns.capacity, ns.used),
            ns.blocks >= ns.capacity && ns.capacity >= ns.used,
        );
        // NVM 1.1 Identify Namespace: FLBAS selects one of the NLBAF + 1 formats.
        ident(
            cases,
            format!("{name} ns {nsid} NVMe 1.0 FLBAS"),
            "a format index within NLBAF",
            format!("format {} of {}", ns.format, ns.formats as u32 + 1),
            ns.format <= ns.formats,
        );
        // NVM 1.1 Identify Namespace NMIC bit 0: set when more than one controller may reach it.
        ident(
            cases,
            format!("{name} ns {nsid} {}", rev::CMIC_FIELD.name()),
            if shared_expected {
                "shared (NMIC bit 0): more than one controller can reach it"
            } else {
                "reported"
            },
            format!("CMIC {:#04x}, NMIC {:#04x}", info.cmic, ns.nmic),
            !shared_expected || ns.nmic & NMIC_SHARED != 0 || !rev::CMIC_FIELD.defined_in(vs),
        );
        // Base 2.1 Figure 319 RESCAP bit 7: a revision 1.3 or later controller sets IEKS when the
        // namespace supports reservations.
        let ieks_due = ns.rescap != 0 && rev::IEKS.defined_in(vs);
        ident(
            cases,
            format!("{name} ns {nsid} {}", rev::IEKS.name()),
            if ieks_due {
                "RESCAP bit 7 set"
            } else {
                "reported (no reservations, or before 1.3)"
            },
            format!(
                "RESCAP {:#04x}, DLFEAT {:#04x}, ANAGRPID {}",
                ns.rescap, ns.dlfeat, ns.anagrpid
            ),
            !ieks_due || ns.rescap & RESCAP_IEKS != 0,
        );
        // Base 2.1 5.1.13.2.3 and Figure 315: the list holds at least one of EUI-64, NGUID or UUID,
        // no type twice, and copies of the Identify Namespace EUI64 and NGUID.
        let mut buf = vec![0u8; 4096];
        let done = c.identify_status(controller::CNS_NS_DESCRIPTORS, nsid, &mut buf);
        let label = format!("{name} ns {nsid}");
        if !rev_case(
            cases,
            "nvme-ident",
            &label,
            &rev::NS_DESCRIPTORS,
            vs,
            &[SUCCESS],
            "success",
            &done,
        ) || !done.as_ref().is_ok_and(|d| d.ok())
        {
            unique = false;
            if geometry {
                namespaces.push((nsid, ns));
            }
            continue;
        }
        let list = parse_descriptors(&buf);
        let ids = list.as_ref().map(|l| unique_ids(l)).unwrap_or_default();
        ident(
            cases,
            format!("{label} {} contents", rev::NS_DESCRIPTORS.name()),
            "well formed, a non-zero EUI-64, NGUID or UUID",
            match &list {
                Ok(_) => show_ids(&ids),
                Err(e) => e.clone(),
            },
            list.is_ok() && !ids.is_empty(),
        );
        if let Ok(l) = &list {
            ident(
                cases,
                format!("{label} descriptors copy the {}", rev::NGUID_FIELD.name()),
                "EUI-64 and NGUID descriptors equal Identify Namespace, present only when it is non-zero",
                format!("NGUID {}, EUI-64 {}", hex(&ns.nguid), hex(&ns.eui64)),
                agrees(&ns, l),
            );
        }
        unique &= list.is_ok() && !ids.is_empty();
        if geometry {
            namespaces.push((nsid, ns));
        }
    }

    // Every other path: a second association to this address, and every other address of the
    // subsystem. Each must report the identity this controller does (Base 2.1 Figure 312: SN, MN and
    // SUBNQN name the NVM subsystem; Figure 315: a NID is fixed for the namespace's life).
    let here = path_identity(c);
    let mut others: Vec<(String, SocketAddr)> =
        vec![(format!("second association at {}", t.addrs[0]), t.addrs[0])];
    others.extend(t.addrs[1..].iter().map(|&a| (format!("path {a}"), a)));
    for (label, addr) in others {
        let there = connect_path(addr, &t.subnqn).and_then(|mut o| path_identity(&mut o));
        let (actual, passed) = match (&here, &there) {
            (Ok(h), Ok(o)) if h == o => ("the same".to_string(), true),
            (Ok(h), Ok(o)) => {
                let mut diff = Vec::new();
                if h.serial != o.serial {
                    diff.push(format!("serial {:?} vs {:?}", h.serial, o.serial));
                }
                if h.model != o.model {
                    diff.push(format!("model {:?} vs {:?}", h.model, o.model));
                }
                if h.nqn != o.nqn {
                    diff.push(format!("NQN {} vs {}", h.nqn, o.nqn));
                }
                if h.namespaces != o.namespaces {
                    diff.push("namespace IDs differ".into());
                }
                (diff.join(", "), false)
            }
            (Err(e), _) | (_, Err(e)) => (e.clone(), false),
        };
        unique &= passed;
        ident(
            cases,
            format!("{name} {label}"),
            "the same serial, model, NQN and namespace IDs",
            actual,
            passed,
        );
    }

    Some(Facts {
        ctrl: info,
        cap,
        vs,
        active,
        namespaces,
        unique_ids: unique,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let ns = controller::parse_namespace(&d);
        assert!(agrees(&ns, &[(NIDT_NGUID, vec![9; 16])]));
        assert!(!agrees(&ns, &[(NIDT_UUID, vec![9; 16])]));
        // An EUI-64 descriptor while Identify Namespace reports none.
        assert!(!agrees(
            &ns,
            &[(NIDT_NGUID, vec![9; 16]), (NIDT_EUI64, vec![1; 8])]
        ));
    }
}
