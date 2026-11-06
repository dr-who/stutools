//! The chapters a report groups its sections into, and what each standard is, in plain words for a
//! reader who has not read it. SPC and SBC are judged at each SCSI level (`judged`), so their titles
//! name the level's edition; iSCSI, host behaviour and NVMe hold at every level.

use super::scsi_level::ScsiLevel;

/// The chapters of a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Iscsi,
    Spc,
    Sbc,
    Hosts,
    Nvme,
}

impl Part {
    /// Whether the part's verdicts depend on the SCSI level.
    pub fn by_level(self) -> bool {
        matches!(self, Part::Spc | Part::Sbc)
    }

    /// The chapter title; at a level, SPC and SBC name its edition.
    pub fn title(self, level: ScsiLevel) -> String {
        match self {
            Part::Iscsi => "iSCSI transport and task management".into(),
            Part::Spc => format!("SPC-{}: SCSI Primary Commands", level.spc()),
            Part::Sbc => format!("SBC-{}: SCSI Block Commands", level.sbc()),
            Part::Hosts => "What hosts need".into(),
            Part::Nvme => "NVMe over TCP".into(),
        }
    }

    /// What the standard is and what of it spit tests.
    pub fn about(self) -> &'static str {
        match self {
            Part::Iscsi => "iSCSI (RFC 7143, with RFC 7144 for the SCSI features) carries SCSI over TCP: a host logs in to a target, authenticates with CHAP, negotiates how the session behaves, and then sends commands, data and task management functions in protocol data units. The task management functions themselves (abort a command, reset a logical unit, query a task) are defined by SAM-6, the SCSI Architecture Model. These rules do not change with the SCSI level.",
            Part::Spc => "SPC, SCSI Primary Commands, defines what every SCSI device shares, whatever its type: INQUIRY and the vital product data (VPD) pages that say what a device is and how it is named, REPORT LUNS, REQUEST SENSE and sense data, mode and log pages, REPORT SUPPORTED OPERATION CODES, persistent reservations (which hosts may use a shared disk, the basis of Failover Clustering), and asymmetric logical unit access (ALUA: which paths are active, the basis of multipath). Its editions are SPC-3 (2005), SPC-4 (2015), SPC-5 (2019) and SPC-6 (2025); each later edition adds commands and pages and makes some older ones obsolete.",
            Part::Sbc => "SBC, SCSI Block Commands, defines what a block device, a disk, does: READ and WRITE in their CDB sizes, READ CAPACITY, SYNCHRONIZE CACHE, VERIFY, WRITE SAME, COMPARE AND WRITE, logical block provisioning (thin provisioning, UNMAP, the TRIM and discard of Windows and Linux), protection information, ROD token copy (Windows ODX), the block VPD and mode pages, and since SBC-4 the feature sets a device can claim. Its editions are SBC-3 (2014), SBC-4 (2021) and SBC-5 (2025).",
            Part::Hosts => "What Windows and Linux actually send when a disk comes online, replayed from packet captures, and what Windows MPIO, NTFS, TRIM and Failover Clustering need of a disk, judged from the SPC and SBC results.",
            Part::Nvme => "NVMe over TCP: NVM Express Base 2.1, the NVM Command Set 1.1 and the TCP Transport 1.1. Discovery, Connect, controller properties, Identify, log pages, features, reads and writes, asymmetric namespace access (ANA, NVMe's multipath) and reservations. NVMe is not SCSI, so the SCSI level does not apply.",
        }
    }
}
