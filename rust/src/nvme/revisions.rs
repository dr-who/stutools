//! The NVM Express revision that introduced each command, log page, feature and field the compliance
//! cases test, and the one that made it obsolete if any. A case is judged against the revision the
//! controller implements (its VS property): a capability newer than that may be refused, and is held
//! to the specification once the controller claims a revision that has it. Each case names the
//! revision ("NVMe 1.4 ANA log page").
//!
//! `TEXT` rows are stated in the NVM Express Base Specification 2.1 (the section is in the comment);
//! `HISTORY` rows come from the NVMe revision history, which the 2.1 text does not restate.

/// A revision as major, minor, tertiary (VS bits 31:16, 15:8, 7:0).
pub type Version = (u16, u8, u8);

/// Where a row's revision comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Text,
    History,
}
use Source::{History as HISTORY, Text as TEXT};

/// One row: what is tested, the revision that introduced it, the one from which a controller must
/// support it (later than `min` for what began optional), and the one that made it obsolete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rev {
    pub item: &'static str,
    pub min: Version,
    pub due: Version,
    pub max: Option<Version>,
    pub source: Source,
}

const fn rev(item: &'static str, min: Version, source: Source) -> Rev {
    Rev {
        item,
        min,
        due: min,
        max: None,
        source,
    }
}

/// A row that became mandatory at `due`, after it was introduced.
const fn mandatory_from(r: Rev, due: Version) -> Rev {
    Rev { due, ..r }
}

// Identify.
pub const IDENTIFY_CONTROLLER: Rev = rev("Identify Controller", (1, 0, 0), HISTORY);
pub const IDENTIFY_NAMESPACE: Rev = rev("Identify Namespace", (1, 0, 0), HISTORY);
/// Base 2.1 5.1.13: the CNS field became two bits, and CNS 02h defined, in revision 1.1.
pub const ACTIVE_NAMESPACES: Rev = rev("Active Namespace ID list", (1, 1, 0), TEXT);
pub const NS_DESCRIPTORS: Rev = rev(
    "Namespace Identification Descriptor list",
    (1, 3, 0),
    HISTORY,
);
/// Base 2.1 Figure 312, VER: revision 1.2 or later reports a non-zero value.
pub const VER_FIELD: Rev = rev("VER field", (1, 2, 0), TEXT);
/// Base 2.1 Figure 312, SUBNQN: mandatory for revision 1.2.1 or later.
pub const SUBNQN_FIELD: Rev = rev("SUBNQN field", (1, 2, 1), TEXT);
pub const CMIC_FIELD: Rev = rev("CMIC and NMIC fields", (1, 1, 0), HISTORY);
pub const NGUID_FIELD: Rev = rev("NGUID field", (1, 2, 0), HISTORY);
pub const DLFEAT_FIELD: Rev = rev("DLFEAT field", (1, 3, 0), HISTORY);
/// Base 2.1 Figure 319 (NVM 1.1 Identify Namespace) RESCAP bit 7: IEKS shall be set by a revision
/// 1.3 or later controller.
pub const IEKS: Rev = rev("Ignore Existing Key (IEKS)", (1, 3, 0), TEXT);

// Log pages.
/// Base 2.1 Figure 31 note 3: Supported Log Pages and Commands Supported and Effects are mandatory,
/// "optional for NVM Express revision 1.4 and earlier", so mandatory from 2.0.
pub const LOG_SUPPORTED: Rev = mandatory_from(
    rev("Supported Log Pages log page", (2, 0, 0), HISTORY),
    (2, 0, 0),
);
pub const LOG_EFFECTS: Rev = mandatory_from(
    rev(
        "Commands Supported and Effects log page",
        (1, 2, 0),
        HISTORY,
    ),
    (2, 0, 0),
);
pub const LOG_ERROR: Rev = rev("Error Information log page", (1, 0, 0), HISTORY);
pub const LOG_SMART: Rev = rev("SMART / Health log page", (1, 0, 0), HISTORY);
pub const LOG_FIRMWARE: Rev = rev("Firmware Slot log page", (1, 0, 0), HISTORY);
pub const LOG_ANA: Rev = rev("ANA log page", (1, 4, 0), HISTORY);
pub const LOG_RESERVATION: Rev = rev("Reservation Notification log page", (1, 1, 0), HISTORY);

// Features.
pub const FEAT_VWC: Rev = rev("Volatile Write Cache feature", (1, 0, 0), HISTORY);
pub const FEAT_QUEUES: Rev = rev("Number of Queues feature", (1, 0, 0), HISTORY);
pub const FEAT_AEC: Rev = rev("Async Event Configuration feature", (1, 0, 0), HISTORY);
pub const FEAT_KATO: Rev = rev("Keep Alive Timer feature", (1, 2, 1), HISTORY);
pub const FEAT_HOST_ID: Rev = rev("Host Identifier feature", (1, 1, 0), HISTORY);
pub const FEAT_RESV_MASK: Rev = rev("Reservation Notification Mask feature", (1, 1, 0), HISTORY);
pub const FEAT_RESV_PERSIST: Rev = rev("Reservation Persistence feature", (1, 1, 0), HISTORY);

// Admin commands.
pub const ABORT: Rev = rev("Abort", (1, 0, 0), HISTORY);
pub const ASYNC_EVENT: Rev = rev("Asynchronous Event Request", (1, 0, 0), HISTORY);
pub const KEEP_ALIVE: Rev = rev("Keep Alive", (1, 2, 1), HISTORY);

// NVM command set.
pub const READ_WRITE: Rev = rev("Read and Write", (1, 0, 0), HISTORY);
pub const FUA: Rev = rev("Force Unit Access", (1, 0, 0), HISTORY);
pub const FLUSH: Rev = rev("Flush", (1, 0, 0), HISTORY);
pub const COMPARE: Rev = rev("Compare", (1, 0, 0), HISTORY);
pub const DATASET_MANAGEMENT: Rev = rev("Dataset Management", (1, 0, 0), HISTORY);
pub const WRITE_ZEROES: Rev = rev("Write Zeroes", (1, 1, 0), HISTORY);
pub const RESERVATIONS: Rev = rev("Reservations", (1, 1, 0), HISTORY);
/// The extended Reservation Report and its 128-bit host identifiers came with NVMe over Fabrics.
pub const RESERVATION_EDS: Rev = rev(
    "Reservation Report extended data structure",
    (1, 2, 1),
    HISTORY,
);

/// Every row, for the table's own tests.
#[cfg(test)]
const ALL: [Rev; 35] = [
    LOG_SUPPORTED,
    LOG_EFFECTS,
    IDENTIFY_CONTROLLER,
    IDENTIFY_NAMESPACE,
    ACTIVE_NAMESPACES,
    NS_DESCRIPTORS,
    VER_FIELD,
    SUBNQN_FIELD,
    CMIC_FIELD,
    NGUID_FIELD,
    DLFEAT_FIELD,
    IEKS,
    LOG_ERROR,
    LOG_SMART,
    LOG_FIRMWARE,
    LOG_ANA,
    LOG_RESERVATION,
    FEAT_VWC,
    FEAT_QUEUES,
    FEAT_AEC,
    FEAT_KATO,
    FEAT_HOST_ID,
    FEAT_RESV_MASK,
    FEAT_RESV_PERSIST,
    ABORT,
    ASYNC_EVENT,
    KEEP_ALIVE,
    READ_WRITE,
    FUA,
    FLUSH,
    COMPARE,
    DATASET_MANAGEMENT,
    WRITE_ZEROES,
    RESERVATIONS,
    RESERVATION_EDS,
];

/// The revision a VS property value names.
pub fn version(vs: u32) -> Version {
    ((vs >> 16) as u16, (vs >> 8) as u8, vs as u8)
}

pub fn show(v: Version) -> String {
    if v.2 == 0 {
        format!("{}.{}", v.0, v.1)
    } else {
        format!("{}.{}.{}", v.0, v.1, v.2)
    }
}

impl Rev {
    /// Whether the revision the controller implements (`vs`) requires this row.
    pub fn defined_in(&self, vs: u32) -> bool {
        let v = version(vs);
        v >= self.due && self.max.is_none_or(|max| v < max)
    }

    /// The row as a case name: "NVMe 1.4 ANA log page".
    pub fn name(&self) -> String {
        format!("NVMe {} {}", show(self.min), self.item)
    }

    /// Why a refusal is acceptable when the controller's revision does not require the row.
    pub fn absent(&self, vs: u32) -> String {
        if self.due == self.min {
            format!(
                "not in NVMe {}, which the controller implements",
                show(version(vs))
            )
        } else {
            format!(
                "optional before NVMe {}; the controller implements {}",
                show(self.due),
                show(version(vs))
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_pin_the_revisions() {
        assert_eq!(LOG_ANA.min, (1, 4, 0));
        assert_eq!(NS_DESCRIPTORS.min, (1, 3, 0));
        assert_eq!(RESERVATIONS.min, (1, 1, 0));
        assert_eq!(KEEP_ALIVE.min, (1, 2, 1));
        assert_eq!(SUBNQN_FIELD.source, Source::Text);
        assert_eq!(LOG_ANA.name(), "NVMe 1.4 ANA log page");
        assert_eq!(KEEP_ALIVE.name(), "NVMe 1.2.1 Keep Alive");
    }

    #[test]
    fn a_row_is_defined_from_its_revision_on() {
        assert!(!LOG_ANA.defined_in(0x0001_0300));
        assert!(LOG_ANA.defined_in(0x0001_0400));
        assert!(LOG_ANA.defined_in(0x0002_0000));
        assert!(!KEEP_ALIVE.defined_in(0x0001_0200));
        assert!(KEEP_ALIVE.defined_in(0x0001_0201));
        let gone = Rev {
            max: Some((2, 0, 0)),
            ..LOG_ERROR
        };
        assert!(!gone.defined_in(0x0002_0000));
        assert_eq!(
            LOG_ANA.absent(0x0001_0300),
            "not in NVMe 1.3, which the controller implements"
        );
        // Introduced in 1.2, required from 2.0.
        assert!(!LOG_EFFECTS.defined_in(0x0001_0400));
        assert!(LOG_EFFECTS.defined_in(0x0002_0000));
        assert_eq!(
            LOG_EFFECTS.name(),
            "NVMe 1.2 Commands Supported and Effects log page"
        );
        assert!(LOG_EFFECTS
            .absent(0x0001_0400)
            .starts_with("optional before NVMe 2.0"));
    }

    #[test]
    fn every_row_is_named_once() {
        let mut items: Vec<&str> = ALL.iter().map(|r| r.item).collect();
        items.sort_unstable();
        items.dedup();
        assert_eq!(items.len(), ALL.len());
    }
}
