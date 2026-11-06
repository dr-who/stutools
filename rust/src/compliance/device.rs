//! A device a run tested, as the report describes it: which target it is, the paths it was reached
//! through, and what it says it is (vendor, product, revision, serial number, capacity). The iSCSI
//! and NVMe drivers record one per logical unit or namespace they test.

/// One device under test.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    /// The name its cases begin with.
    pub name: String,
    /// "iSCSI" or "NVMe/TCP".
    pub transport: &'static str,
    /// The target IQN or subsystem NQN.
    pub target: String,
    /// Every portal or address it was reached through.
    pub paths: Vec<String>,
    pub vendor: String,
    pub product: String,
    pub revision: String,
    pub serial: String,
    /// Logical blocks and bytes per block.
    pub blocks: u64,
    pub block_len: u32,
}

impl Device {
    /// The capacity in bytes.
    pub fn bytes(&self) -> u64 {
        self.blocks.saturating_mul(u64::from(self.block_len))
    }
}
