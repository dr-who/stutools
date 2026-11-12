//! NVMe submission and completion queue entries (NVMe base specification): a 64-byte command and a
//! 16-byte completion. Fields are little-endian.

pub const OP_FLUSH: u8 = 0x00;
pub const OP_WRITE: u8 = 0x01;
pub const OP_READ: u8 = 0x02;
pub const OP_IDENTIFY: u8 = 0x06;
pub const OP_GET_LOG_PAGE: u8 = 0x02;
pub const OP_KEEP_ALIVE: u8 = 0x18;
pub const OP_ABORT: u8 = 0x08;
pub const OP_SET_FEATURES: u8 = 0x09;
pub const OP_GET_FEATURES: u8 = 0x0a;
pub const OP_ASYNC_EVENT: u8 = 0x0c;
/// NVM command set I/O opcodes beyond Read, Write and Flush.
pub const OP_COMPARE: u8 = 0x05;
pub const OP_WRITE_ZEROES: u8 = 0x08;
pub const OP_DATASET_MANAGEMENT: u8 = 0x09;
pub const OP_RESV_REGISTER: u8 = 0x0d;
pub const OP_RESV_REPORT: u8 = 0x0e;
pub const OP_RESV_ACQUIRE: u8 = 0x11;
pub const OP_RESV_RELEASE: u8 = 0x15;
/// Fabrics commands share one opcode; the fabric command type sits in byte 4.
pub const OP_FABRICS: u8 = 0x7f;

pub const FCTYPE_PROPERTY_SET: u8 = 0x00;
pub const FCTYPE_CONNECT: u8 = 0x01;
pub const FCTYPE_PROPERTY_GET: u8 = 0x04;

/// SGL descriptor types used in the data pointer: inline (in-capsule) data, and data the controller
/// moves with R2T and C2H/H2C PDUs.
pub const SGL_INLINE: u8 = 0x01;
pub const SGL_TRANSPORT: u8 = 0x00;

/// PSDT in the flags byte: the data pointer is an SGL.
const PSDT_SGL: u8 = 0x40;

/// Byte offset of the data pointer (DPTR) and of the SGL descriptor's length and type.
const DPTR: usize = 24;
const SGL_LENGTH: usize = 32;
const SGL_TYPE: usize = 39;

/// A 64-byte submission queue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command(pub [u8; 64]);

impl Command {
    pub fn new(opcode: u8, cid: u16) -> Self {
        let mut b = [0u8; 64];
        b[0] = opcode;
        b[2..4].copy_from_slice(&cid.to_le_bytes());
        Self(b)
    }

    #[cfg(test)]
    pub fn cid(&self) -> u16 {
        u16::from_le_bytes([self.0[2], self.0[3]])
    }

    pub fn set_nsid(&mut self, nsid: u32) {
        self.set_u32(4, nsid);
    }

    pub fn set_u8(&mut self, at: usize, value: u8) {
        self.0[at] = value;
    }

    pub fn set_u16(&mut self, at: usize, value: u16) {
        self.0[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    pub fn set_u32(&mut self, at: usize, value: u32) {
        self.0[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn set_u64(&mut self, at: usize, value: u64) {
        self.0[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }

    /// Describes `len` bytes of data as an SGL. `inline` data travels in the capsule itself; any other
    /// data is moved by the controller with R2T and data PDUs.
    pub fn set_data(&mut self, len: usize, inline: bool) {
        self.0[1] = PSDT_SGL;
        self.0[DPTR..DPTR + 16].fill(0);
        self.set_u32(SGL_LENGTH, len as u32);
        self.0[SGL_TYPE] = if inline { SGL_INLINE } else { SGL_TRANSPORT };
    }
}

/// A 16-byte completion queue entry, decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Completion {
    /// Command-specific DW0. Property Get of an 8-byte property uses the whole 64-bit value.
    pub dw0: u32,
    pub wide: u64,
    pub sq_head: u16,
    pub cid: u16,
    /// The status field: the phase bit in bit 0, then the status code (bits 1-8), the status code
    /// type (bits 9-11) and the more and do-not-retry bits.
    pub status: u16,
}

impl Completion {
    pub fn parse(b: &[u8; 16]) -> Self {
        Self {
            dw0: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            wide: u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
            sq_head: u16::from_le_bytes([b[8], b[9]]),
            cid: u16::from_le_bytes([b[12], b[13]]),
            status: u16::from_le_bytes([b[14], b[15]]),
        }
    }

    /// True when the command completed with no error (the phase bit is ignored).
    pub fn ok(&self) -> bool {
        self.status >> 1 == 0
    }

    /// The status code type and status code, for error messages.
    pub fn code(&self) -> (u8, u8) {
        (
            ((self.status >> 9) & 0x7) as u8,
            ((self.status >> 1) & 0xff) as u8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_put_the_cid_and_nsid_little_endian() {
        let mut c = Command::new(OP_READ, 0x0102);
        c.set_nsid(0x0a0b_0c0d);
        assert_eq!(c.0[0], OP_READ);
        assert_eq!(&c.0[2..4], &[0x02, 0x01]);
        assert_eq!(&c.0[4..8], &[0x0d, 0x0c, 0x0b, 0x0a]);
        assert_eq!(c.cid(), 0x0102);
    }

    #[test]
    fn data_sgl_records_length_and_type() {
        let mut c = Command::new(OP_IDENTIFY, 1);
        c.set_data(4096, false);
        assert_eq!(&c.0[32..36], &4096u32.to_le_bytes());
        assert_eq!(c.0[39], SGL_TRANSPORT);
        c.set_data(1024, true);
        assert_eq!(c.0[39], SGL_INLINE);
    }

    #[test]
    fn completion_status_ignores_the_phase_bit() {
        let mut b = [0u8; 16];
        b[12] = 7;
        b[14] = 0x01; // phase set, status zero
        let c = Completion::parse(&b);
        assert!(c.ok());
        assert_eq!(c.cid, 7);
        b[14] = 0x04 << 1; // status code 4 in SCT 0
        assert_eq!(Completion::parse(&b).code(), (0, 4));
        assert!(!Completion::parse(&b).ok());
    }
}
