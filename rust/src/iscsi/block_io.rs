//! The `io` cases of `--compliance`: the data commands every host sends (SBC-5 5.16 to 5.53): READ
//! and WRITE (10) and (16) with the data read back, FUA (which NTFS uses for its log and metadata),
//! out-of-range and zero-length transfers, a 1 MiB read, a transfer over the Block Limits maximum,
//! SYNCHRONIZE CACHE (10) and (16) (the NTFS and ext4 flush), VERIFY (10) and (16) (Windows chkdsk /r
//! sends them) and WRITE SAME (10) and (16). Data-changing cases use the LUN's second-to-last block and
//! write its original data back.

use super::cases::{expect, record, Want, REFUSED};
use super::disk::Disk;
use super::lun_access::{pattern, read_block, write_block};
use super::port::Port;
use super::seen::Seen;
use super::session::Outcome;
use crate::compliance::Case;
use crate::Result;

const SECTION: &str = "io";

/// A 10-byte READ, WRITE, VERIFY or SYNCHRONIZE CACHE CDB: byte 1 flags, a 32-bit LBA, a 16-bit count
/// (SBC-5 tables 80, 134, 126, 120).
pub(super) fn cdb10(op: u8, flags: u8, lba: u32, count: u16) -> [u8; 10] {
    let l = lba.to_be_bytes();
    let c = count.to_be_bytes();
    [op, flags, l[0], l[1], l[2], l[3], 0, c[0], c[1], 0]
}

/// A 16-byte READ, WRITE, VERIFY, WRITE SAME or SYNCHRONIZE CACHE CDB: a 64-bit LBA and a 32-bit count
/// (SBC-5 tables 83, 136, 132, 150, 121).
pub(super) fn cdb16(op: u8, flags: u8, lba: u64, count: u32) -> [u8; 16] {
    let mut cdb = [0u8; 16];
    cdb[0] = op;
    cdb[1] = flags;
    cdb[2..10].copy_from_slice(&lba.to_be_bytes());
    cdb[10..14].copy_from_slice(&count.to_be_bytes());
    cdb
}

/// FUA in byte 1 of the read and write CDBs (SBC-5 table 80).
const FUA: u8 = 0x08;
/// IMMED in byte 1 of SYNCHRONIZE CACHE (SBC-5 tables 120, 121).
const IMMED: u8 = 0x02;

/// What the data cases found, for the Windows cases.
pub(super) struct Findings {
    /// SYNCHRONIZE CACHE and a FUA write both work: what NTFS needs to order its log.
    pub flush: bool,
}

pub(super) fn run(
    s: &mut Port,
    name: &str,
    disk: &Disk,
    cases: &mut Vec<Case>,
    seen: &mut Seen,
) -> Findings {
    let mut t = Io {
        s,
        name,
        cases,
        seen,
    };
    let len = disk.block_len as usize;
    let last = disk.last_lba;
    let Ok(target32) = u32::try_from(last.saturating_sub(1)) else {
        record(
            t.cases,
            SECTION,
            format!("{name} LUN size"),
            "32-bit LBAs",
            "larger",
            false,
        );
        return Findings { flush: false };
    };
    let original = match read_block(t.s, target32, len) {
        Ok(d) => d,
        Err(err) => {
            record(
                t.cases,
                SECTION,
                format!("{name} read the test block"),
                "GOOD",
                err,
                false,
            );
            return Findings { flush: false };
        }
    };
    let target = target32 as u64;
    let p = pattern(len, 0xc3);
    let q = pattern(len, 0x3c);

    // SBC-5 5.40, 5.16: WRITE (10) then READ (10) returns what was written.
    let w = t.cmd(
        "WRITE (10)",
        "WRITE (10)",
        &[Want::Good],
        cdb10(0x2a, 0, target32, 1).as_ref(),
        0,
        Some(&p),
    );
    let r = t.read_back(
        "READ (10)",
        "READ (10) returns what WRITE (10) wrote",
        &cdb10(0x28, 0, target32, 1),
        len,
        &p,
    );
    // SBC-5 5.42, 5.18: the same with the 16-byte CDBs, which hosts use past 2 TiB.
    t.cmd(
        "WRITE (16)",
        "WRITE (16)",
        &[Want::Good],
        &cdb16(0x8a, 0, target, 1),
        0,
        Some(&q),
    );
    t.read_back(
        "READ (16)",
        "READ (16) returns what WRITE (16) wrote",
        &cdb16(0x88, 0, target, 1),
        len,
        &q,
    );
    // SBC-5 5.40: FUA writes to the medium before GOOD. NTFS sends it when DPOFUA is set.
    let fua = t.cmd(
        "WRITE (10)",
        "WRITE (10) with FUA",
        &[Want::Good],
        &cdb10(0x2a, FUA, target32, 1),
        0,
        Some(&p),
    );
    t.seen.fact("FUA write", fua);
    t.read_back(
        "READ (10)",
        "READ (10) with FUA returns the data",
        &cdb10(0x28, FUA, target32, 1),
        len,
        &p,
    );
    if let Err(err) = write_block(t.s, target32, &original) {
        record(
            t.cases,
            SECTION,
            format!("{name} restore the test block"),
            "written back",
            err,
            false,
        );
    }
    let _ = (w, r);

    // SBC-5 4.5 and table 48: an LBA past the capacity is LOGICAL BLOCK ADDRESS OUT OF RANGE (21h/00h).
    let out = [Want::Ascq(0x5, 0x21, 0x00)];
    if let Ok(past) = u32::try_from(last + 1) {
        t.cmd(
            "READ (10)",
            "READ (10) past the last LBA",
            &out,
            &cdb10(0x28, 0, past, 1),
            len as u32,
            None,
        );
        t.cmd(
            "WRITE (10)",
            "WRITE (10) past the last LBA",
            &out,
            &cdb10(0x2a, 0, past, 1),
            0,
            Some(&original),
        );
    }
    t.cmd(
        "READ (10)",
        "READ (10) of two blocks from the last LBA",
        &out,
        &cdb10(0x28, 0, last as u32, 2),
        2 * len as u32,
        None,
    );
    t.cmd(
        "READ (16)",
        "READ (16) past the last LBA",
        &out,
        &cdb16(0x88, 0, last + 1, 1),
        len as u32,
        None,
    );
    // SBC-5 5.16: a TRANSFER LENGTH of zero transfers nothing and is not an error.
    t.cmd(
        "READ (10)",
        "READ (10) of zero blocks",
        &[Want::Good],
        &cdb10(0x28, 0, target32, 0),
        0,
        None,
    );
    t.cmd(
        "WRITE (10)",
        "WRITE (10) of zero blocks",
        &[Want::Good],
        &cdb10(0x2a, 0, target32, 0),
        0,
        None,
    );

    // A 1 MiB read, as hosts send for sequential I/O, limited by MAXIMUM TRANSFER LENGTH when it is less,
    // and by what this host's SG_IO carries on a local disk.
    let limits = disk.limits().unwrap_or_default();
    let carried =
        t.s.max_transfer()
            .map(|b| (b / disk.block_len.max(1)).max(1));
    let mib = ((1u32 << 20) / disk.block_len.max(1)).max(1);
    let mut blocks = if limits.max_transfer > 0 {
        mib.min(limits.max_transfer)
    } else {
        mib
    };
    if let Some(c) = carried {
        blocks = blocks.min(c);
    }
    t.cmd(
        "READ (16)",
        &format!("READ (16) of {blocks} blocks"),
        &[Want::Good],
        &cdb16(0x88, 0, 0, blocks),
        blocks * disk.block_len,
        None,
    );
    // SBC-5 table 33: more than MAXIMUM TRANSFER LENGTH is an invalid field in the CDB.
    let over = limits.max_transfer as u64 + 1;
    let fits = carried.is_none_or(|c| over <= u64::from(c));
    if limits.max_transfer > 0 && over * len as u64 <= 32 << 20 && over <= last && fits {
        t.cmd(
            "READ (16)",
            "READ (16) over MAXIMUM TRANSFER LENGTH",
            &[Want::Ascq(0x5, 0x24, 0x00)],
            &cdb16(0x88, 0, 0, over as u32),
            (over * len as u64) as u32,
            None,
        );
    }

    // SBC-5 5.33, 5.34: SYNCHRONIZE CACHE of the whole LUN (NUMBER OF LOGICAL BLOCKS 0), with IMMED
    // (a device server without IMMED answers INVALID FIELD IN CDB), and past the capacity.
    let immed_refused = Want::Ascq(0x5, 0x24, 0x00);
    let sync = t.cmd(
        "SYNCHRONIZE CACHE (10)",
        "SYNCHRONIZE CACHE (10) of the whole LUN",
        &[Want::Good],
        &cdb10(0x35, 0, 0, 0),
        0,
        None,
    );
    t.cmd(
        "SYNCHRONIZE CACHE (10)",
        "SYNCHRONIZE CACHE (10) with IMMED",
        &[Want::Good, immed_refused],
        &cdb10(0x35, IMMED, 0, 0),
        0,
        None,
    );
    let mut sync16 = vec![Want::Good];
    sync16.extend(REFUSED);
    t.cmd(
        "SYNCHRONIZE CACHE (16)",
        "SYNCHRONIZE CACHE (16) of the whole LUN",
        &sync16,
        &cdb16(0x91, 0, 0, 0),
        0,
        None,
    );
    let mut sync16_immed = sync16.clone();
    sync16_immed.push(immed_refused);
    let (_, immed) = t.answer(
        "SYNCHRONIZE CACHE (16)",
        "SYNCHRONIZE CACHE (16) with IMMED",
        &sync16_immed,
        &cdb16(0x91, IMMED, 0, 0),
        0,
        None,
    );
    // Discovery's SBC Base 2016 needs IMMED itself, not only a clean refusal of it.
    t.seen.fact(
        "SYNCHRONIZE CACHE (16) IMMED",
        immed.is_some_and(|o| o.status == 0),
    );
    if let Ok(past) = u32::try_from(last + 1) {
        t.cmd(
            "SYNCHRONIZE CACHE (10)",
            "SYNCHRONIZE CACHE (10) past the last LBA",
            &out,
            &cdb10(0x35, 0, past, 1),
            0,
            None,
        );
    }

    // SBC-5 5.36, 5.38: VERIFY with BYTCHK 00b checks the medium and transfers nothing.
    let mut verify = vec![Want::Good];
    verify.extend(REFUSED);
    t.cmd(
        "VERIFY (10)",
        "VERIFY (10), BYTCHK 00b",
        &verify,
        &cdb10(0x2f, 0, target32, 1),
        0,
        None,
    );
    t.cmd(
        "VERIFY (16)",
        "VERIFY (16), BYTCHK 00b",
        &verify,
        &cdb16(0x8f, 0, target, 1),
        0,
        None,
    );
    if t.supported("VERIFY (10)") {
        if let Ok(past) = u32::try_from(last + 1) {
            t.cmd(
                "VERIFY (10)",
                "VERIFY (10) past the last LBA",
                &out,
                &cdb10(0x2f, 0, past, 1),
                0,
                None,
            );
        }
    }

    // SBC-5 5.52, 5.53: WRITE SAME of one block writes it; the read returns the pattern. Then the block
    // is written back.
    let ws10 = {
        let l = target32.to_be_bytes();
        [0x41, 0, l[0], l[1], l[2], l[3], 0, 0, 1, 0]
    };
    let mut same = vec![Want::Good];
    same.extend(REFUSED);
    if t.cmd(
        "WRITE SAME (10)",
        "WRITE SAME (10) of one block",
        &same,
        &ws10,
        0,
        Some(&q),
    ) && t.supported("WRITE SAME (10)")
    {
        t.read_back(
            "WRITE SAME (10)",
            "READ after WRITE SAME (10)",
            &cdb10(0x28, 0, target32, 1),
            len,
            &q,
        );
    }
    if t.cmd(
        "WRITE SAME (16)",
        "WRITE SAME (16) of one block",
        &same,
        &cdb16(0x93, 0, target, 1),
        0,
        Some(&p),
    ) && t.supported("WRITE SAME (16)")
    {
        t.read_back(
            "WRITE SAME (16)",
            "READ after WRITE SAME (16)",
            &cdb10(0x28, 0, target32, 1),
            len,
            &p,
        );
    }
    if let Err(err) = write_block(t.s, target32, &original) {
        record(
            t.cases,
            SECTION,
            format!("{name} restore the test block"),
            "written back",
            err,
            false,
        );
    }
    Findings { flush: sync && fua }
}

struct Io<'a> {
    s: &'a mut Port,
    name: &'a str,
    cases: &'a mut Vec<Case>,
    seen: &'a mut Seen,
}

impl Io<'_> {
    /// Sends one command and records it against `command` in `seen`.
    fn cmd(
        &mut self,
        command: &'static str,
        label: &str,
        wants: &[Want],
        cdb: &[u8],
        read: u32,
        data: Option<&[u8]>,
    ) -> bool {
        self.answer(command, label, wants, cdb, read, data).0
    }

    /// `cmd`, with the answer as well as the verdict.
    fn answer(
        &mut self,
        command: &'static str,
        label: &str,
        wants: &[Want],
        cdb: &[u8],
        read: u32,
        data: Option<&[u8]>,
    ) -> (bool, Option<Outcome>) {
        let result: Result<Outcome> = self.s.command(cdb, read, data);
        let answer = expect(
            self.cases,
            SECTION,
            format!("{} {label}", self.name),
            wants,
            result,
        );
        self.seen.answer(command, SECTION, &answer);
        answer
    }

    fn read_back(
        &mut self,
        command: &'static str,
        label: &str,
        cdb: &[u8],
        len: usize,
        want: &[u8],
    ) -> bool {
        let r = self.s.command(cdb, len as u32, None);
        let (actual, passed) = match &r {
            Ok(o) if o.status == 0 && o.data == want => ("the data written".to_string(), true),
            Ok(o) if o.status == 0 => ("other data".to_string(), false),
            Ok(o) => (super::cases::describe(o), false),
            Err(e) => (e.clone(), false),
        };
        record(
            self.cases,
            SECTION,
            format!("{} {label}", self.name),
            "the data written",
            actual.clone(),
            passed,
        );
        self.seen.note(command, SECTION, true, passed, actual);
        passed
    }

    fn supported(&self, command: &str) -> bool {
        self.seen.probe(command).is_some_and(|p| p.supported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdbs_put_lba_and_count_big_endian() {
        assert_eq!(
            cdb10(0x28, FUA, 0x0102_0304, 0x0506),
            [0x28, 0x08, 1, 2, 3, 4, 0, 5, 6, 0]
        );
        let c = cdb16(0x88, 0, 0x0102_0304_0506_0708, 0x0a0b_0c0d);
        assert_eq!(&c[2..10], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&c[10..14], &[0x0a, 0x0b, 0x0c, 0x0d]);
    }
}
