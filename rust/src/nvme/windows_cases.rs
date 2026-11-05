//! The windows cases of `--compliance` for one NVM subsystem: what Windows needs of it to use it as a
//! disk on more than one path and in a cluster, judged from what the other sections found.
//! Windows sends SCSI commands, which stornvme translates as SCSI-NVMe 1.5 describes.

use super::compliance::Target;
use super::{identify_cases, io_cases, reservation_cases};
use crate::compliance::{case, Case};

/// What Windows needs of this subsystem to use it as a disk on more than one path and in a cluster.
/// Windows sends SCSI commands, which stornvme translates as SCSI-NVMe 1.5 describes: the Device
/// Identification page MPIO matches paths by comes from the namespace IDs (6.1.4), SYNCHRONIZE CACHE
/// from Flush (5.5), PERSISTENT RESERVE from the reservation commands (4.13), UNMAP from Dataset
/// Management with the deallocate attribute (5.6), and the caching mode page's WCE from the Volatile
/// Write Cache feature (6.3.3.2).
pub fn run(
    name: &str,
    t: &Target,
    facts: &identify_cases::Facts,
    io: &io_cases::IoFacts,
    ana: Option<&[String]>,
    resv: &reservation_cases::Verdict,
    cases: &mut Vec<Case>,
) {
    let paths = t.addrs.len() + 1;
    cases.push(case(
        "windows",
        format!(
            "{name} MPIO: Device Identification VPD from the namespace IDs (SCSI-NVMe 1.5 6.1.4)"
        ),
        format!("a non-zero EUI-64, NGUID or UUID, the same on all {paths} paths"),
        if facts.unique_ids {
            "the same".into()
        } else {
            "missing or different".into()
        },
        facts.unique_ids,
    ));
    cases.push(case(
        "windows",
        format!("{name} NTFS: SYNCHRONIZE CACHE as Flush (SCSI-NVMe 1.5 5.5)"),
        "success on every namespace".into(),
        if io.flush {
            "success".into()
        } else {
            "failed".into()
        },
        io.flush,
    ));
    cases.push(case(
        "windows",
        format!(
            "{name} Failover Clustering: PERSISTENT RESERVE as reservations (SCSI-NVMe 1.5 4.13)"
        ),
        "reservations with RTYPE 3h (SCSI type 5h), every case passed".into(),
        if !resv.supported {
            format!(
                "reservations off (ONCS bit 5 {}, RESCAP {:#04x})",
                facts.ctrl.oncs >> 5 & 1,
                facts.namespaces.first().map_or(0, |n| n.1.rescap)
            )
        } else if resv.clustering {
            "supported, every case passed".into()
        } else {
            "supported, but a case failed or the type is missing".into()
        },
        resv.clustering,
    ));
    cases.push(case(
        "windows",
        format!("{name} MPIO path states (ANA)"),
        "reported on every path, or not reported (every path is then used alike)".into(),
        match ana {
            Some(states) if !states.is_empty() => states.join(", "),
            Some(_) => "reported, no namespace listed".into(),
            None => "not reported".into(),
        },
        ana.is_none_or(|s| !s.is_empty()),
    ));
    cases.push(case(
        "windows",
        format!("{name} TRIM: UNMAP as Dataset Management deallocate (SCSI-NVMe 1.5 5.6)"),
        "works, or is not offered".into(),
        match io.trim {
            Some(true) => "works".into(),
            Some(false) => "offered but failed".into(),
            None => "not offered (ONCS bit 2 clear)".into(),
        },
        io.trim != Some(false),
    ));
    let cache = facts.ctrl.vwc & 1 != 0;
    cases.push(case(
        "windows",
        format!("{name} write cache: caching mode page WCE (SCSI-NVMe 1.5 6.3.3.2)"),
        "reported; flushed when present".into(),
        if cache {
            "volatile write cache, Flush works"
        } else {
            "no volatile write cache"
        }
        .into(),
        !cache || io.flush,
    ));
}
