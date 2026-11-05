//! The nvme-ident cases of `--compliance` (Base 2.1 5.1.13 Identify; NVM 1.1 Identify Namespace): the
//! Identify Controller fields a host depends on, each active namespace's Identify Namespace data and
//! its identification descriptors (CNS 03h), and the same identity on every other path. Windows MPIO
//! groups paths by the namespace's unique ID (EUI-64, NGUID or UUID) and the serial number, model and
//! subsystem NQN, so every controller of the subsystem must report the same ones.
//! Each case names the NVMe revision that defines what it tests and is judged against the revision
//! the controller reports. Returns what it learned for the later sections, or None when Identify
//! itself fails.

use super::compliance::Target;
use super::controller::Controller;
use super::identify::{
    self, agrees, parse_descriptors, unique_ids, ControllerInfo, Descriptor, NamespaceInfo,
    NIDT_EUI64, NIDT_NGUID,
};
use super::path_identity_cases;
use super::revisions::{self as rev, Rev};
use super::verdict::{rev_case, SUCCESS};
use crate::compliance::{case, Case};

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
    let info = match c.identify(identify::CNS_CONTROLLER, 0) {
        Ok(d) => identify::parse_controller(&d),
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
    let active = match c.identify(identify::CNS_ACTIVE_NAMESPACES, 0) {
        Ok(d) => identify::parse_active_namespaces(&d),
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
        let ns = match c.identify(identify::CNS_NAMESPACE, nsid) {
            Ok(d) => identify::parse_namespace(&d),
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
        let done = c.identify_status(identify::CNS_NS_DESCRIPTORS, nsid, &mut buf);
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

    unique &= path_identity_cases::run(c, name, t, cases);

    Some(Facts {
        ctrl: info,
        cap,
        vs,
        active,
        namespaces,
        unique_ids: unique,
    })
}
