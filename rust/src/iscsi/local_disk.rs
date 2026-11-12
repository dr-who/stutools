//! `--compliance` on a local disk (`-f /dev/...`): the SCSI sections, sent through SG_IO. The device
//! is opened with O_EXCL for the whole run, so a disk that is mounted, part of an array, or open
//! exclusively by anything else is refused rather than tested; the run writes only blocks it writes
//! back, but it reserves and resets the disk. A dm-multipath device is tested through its paths: each
//! host adapter is an initiator port, so with two the reservation cases act as two hosts
//! (`local_nexus`). The iSCSI-only sections (login, CHAP, sessions, task management, digests) do not
//! run.

use super::commands::Commands;
use super::conformance;
use super::local_lun::LocalLun;
use super::local_nexus::{self, FIRST, SECOND};
use super::path::Path;
use super::scsi;
use crate::compliance::{Case, Device, ScsiLevel};
use std::fs::File;
use std::os::unix::fs::OpenOptionsExt;

/// Adds the cases for each local disk in `devices` to `cases`, the disks not tested to `untested`,
/// and each disk tested to `found`.
pub fn cases(
    devices: &[String],
    levels: &[ScsiLevel],
    cases: &mut Vec<Case>,
    untested: &mut Vec<(String, String)>,
    found: &mut Vec<Device>,
) {
    for device in devices {
        // Held until the disk's cases are done.
        let _exclusive = match File::options()
            .read(true)
            .custom_flags(libc::O_EXCL)
            .open(device)
        {
            Ok(f) => f,
            Err(e) => {
                untested.push((device.clone(), format!("not opened exclusively: {e}")));
                continue;
            }
        };
        let path = Path::local(device);
        match describe(device) {
            Ok((d, health, sizes)) => {
                print_header(&d, &health, sizes);
                found.push(d);
            }
            Err(e) => {
                untested.push((device.clone(), e));
                continue;
            }
        }
        super::cases::NO_RESIDUAL.store(true, std::sync::atomic::Ordering::Relaxed);
        identity(device, cases);
        let second = match local_nexus::initiator_ports(device) {
            Ok(n) if n >= 2 => Some(SECOND),
            _ => None,
        };
        let target = conformance::Target {
            path: &path,
            creds: None,
            standby: false,
            protocol_level: None,
        };
        conformance::run(&[target], FIRST, second, levels, cases);
        reset(device, cases);
        super::cases::NO_RESIDUAL.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// SPC-6 7.7.6, 7.7.19: every path of a multipath disk names the same logical unit (serial number,
/// logical unit designators, capacity), which Windows MPIO and dm-multipath depend on.
fn identity(device: &str, cases: &mut Vec<Case>) {
    let paths = local_nexus::path_devices(device).unwrap_or_default();
    let read = |dev: &str| -> crate::Result<(String, Vec<String>, (u64, u32))> {
        let mut l = LocalLun::open(dev)?;
        let serial = scsi::serial(&mut l)?;
        let mut lu: Vec<String> = scsi::designators(&mut l)?
            .into_iter()
            .filter(|d| d.association == 0)
            .map(|d| format!("{}:{}", d.kind, d.value))
            .collect();
        lu.sort();
        Ok((serial, lu, scsi::capacity(&mut l)?))
    };
    let Some(first) = paths.first() else {
        return;
    };
    let reference = read(first);
    for p in &paths {
        let (actual, passed) = match (read(p), &reference) {
            (Ok(got), Ok(want)) if got == *want => (
                "the same serial, designators and capacity".to_string(),
                true,
            ),
            (Ok(_), Ok(_)) => (format!("differs from {first}"), false),
            (Err(e), _) => (e, false),
            (_, Err(e)) => (e.clone(), false),
        };
        cases.push(crate::compliance::case(
            "identity",
            format!("{device} path {p} names the same logical unit"),
            "the same serial, designators and capacity as every path".into(),
            actual,
            passed,
        ));
    }
}

/// SAM-6 7.8: LOGICAL UNIT RESET, sent the only way a local disk takes one (the host adapter's
/// SG_SCSI_RESET), then every path answers TEST UNIT READY again, as MPIO's path recovery needs. The
/// unit attention a reset leaves (29h) is recorded, not judged: multipathd's path checker may read it
/// first.
fn reset(device: &str, cases: &mut Vec<Case>) {
    let mut paths = local_nexus::path_devices(device).unwrap_or_default();
    if paths.is_empty() {
        paths.push(device.to_string());
    }
    let label = format!("{device} LOGICAL UNIT RESET (SG_SCSI_RESET)");
    let done = LocalLun::open(&paths[0]).and_then(|l| l.reset());
    match &done {
        Ok(()) => cases.push(crate::compliance::case(
            "tmf",
            label,
            "done".into(),
            "done".into(),
            true,
        )),
        // SG_SCSI_RESET resets the device through the host adapter; a refusal is the host's, not a LUN reset
        // the device failed to perform, so it is reported and not judged.
        Err(err) => cases.push(crate::compliance::info(
            "tmf",
            label,
            format!("not run: SG_SCSI_RESET was refused by the host on this path ({err}), so no LUN reset was sent"),
        )),
    }
    for p in &paths {
        let mut attentions = Vec::new();
        let mut answer = Err("no answer".to_string());
        if let Ok(mut l) = LocalLun::open(p) {
            for _ in 0..4 {
                answer = l.command(&[0u8; 6], 0, None);
                match &answer {
                    Ok(o) if o.status == 2 && o.sense.get(2).is_some_and(|k| k & 0x0f == 6) => {
                        attentions.push(format!(
                            "{:02x}h/{:02x}h",
                            o.sense.get(12).copied().unwrap_or(0),
                            o.sense.get(13).copied().unwrap_or(0)
                        ));
                    }
                    _ => break,
                }
            }
        }
        let good = matches!(&answer, Ok(o) if o.status == 0);
        cases.push(crate::compliance::case(
            "tmf",
            format!("{device} path {p} answers after the reset"),
            "TEST UNIT READY GOOD".into(),
            if good {
                "GOOD".into()
            } else {
                format!("{:?}", answer.map(|o| o.status))
            },
            good,
        ));
        cases.push(crate::compliance::info(
            "tmf",
            format!("{device} path {p} unit attention after the reset"),
            if attentions.is_empty() {
                "none seen".into()
            } else {
                attentions.join(", ")
            },
        ));
    }
}

/// The disk as the report names it: INQUIRY, the serial number and READ CAPACITY, and every path.
/// The device's identity, its health (TEST UNIT READY) and its block sizes.
fn describe(device: &str) -> crate::Result<(Device, String, scsi::BlockSizes)> {
    let first = local_nexus::device_for(device, FIRST, false)?;
    let mut lun = LocalLun::open(&first)?;
    let inquiry = scsi::inquiry(&mut lun)?;
    let serial = scsi::serial(&mut lun).unwrap_or_default();
    let (last, block_len) = scsi::capacity(&mut lun)?;
    let sizes = scsi::block_sizes(&mut lun)?;
    let health = health(&mut lun);
    let paths = local_nexus::all_paths(device)?;
    let sas = format!(
        "/sys/block/{}/device/sas_address",
        first.trim_start_matches("/dev/")
    );
    let transport = if std::path::Path::new(&sas).exists() {
        "SAS"
    } else {
        "SCSI"
    };
    let device = Device {
        name: device.to_string(),
        transport,
        target: device.to_string(),
        paths,
        vendor: inquiry.vendor,
        product: inquiry.product,
        revision: inquiry.revision,
        serial,
        blocks: last.saturating_add(1),
        block_len,
    };
    Ok((device, health, sizes))
}

/// What TEST UNIT READY says about the device now: ready, or the sense it reports.
fn health(lun: &mut LocalLun) -> String {
    match lun.command(&[0; 6], 0, None) {
        Ok(o) if o.status == 0 => "ready (TEST UNIT READY GOOD)".to_string(),
        Ok(o) if o.sense.len() >= 14 => format!(
            "not ready: sense key {:#x}, ASC/ASCQ {:02x}/{:02x}",
            o.sense[2] & 0x0f,
            o.sense[12],
            o.sense[13]
        ),
        Ok(o) => format!("not ready: status {:#x}", o.status),
        Err(e) => format!("no answer: {e}"),
    }
}

/// The device header, in the order the report reads it: model, name, details, health, size, block sizes.
fn print_header(d: &Device, health: &str, sizes: scsi::BlockSizes) {
    let per = 1u64 << sizes.per_physical_exponent;
    println!("model:       {} {} {}", d.vendor, d.product, d.revision);
    println!("name:        {}", d.name);
    println!(
        "details:     {}, serial {}, paths {}",
        d.transport,
        d.serial,
        d.paths.join(", ")
    );
    println!("health:      {health}");
    println!(
        "size:        {} GB ({} logical blocks of {} B)",
        d.bytes() / 1_000_000_000,
        d.blocks,
        d.block_len
    );
    println!(
        "block sizes: {} B logical, {} B physical ({} logical per physical), lowest aligned LBA {}",
        sizes.logical,
        sizes.physical(),
        per,
        sizes.lowest_aligned
    );
}
