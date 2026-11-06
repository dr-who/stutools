//! What a run means for the uses a block device is bought for: Windows MPIO with NTFS, Failover
//! Clustering, VMware ESXi, Linux dm-multipath, shared-disk cluster file systems and the others. Each
//! use names the commands it needs and the cases that test them: a section, words one of which the
//! case name holds, and words that set a case aside when the use never meets that situation (cluster
//! nodes each have their own initiator name, so the cases about two ports sharing one name do not
//! touch Failover Clustering). A use's verdict on a transport is the worst of its cases there: at
//! risk when one fails, ready with warnings when one warns, ready when all pass. A need no case of
//! that transport tests leaves the use "partly tested", and a use none of whose needs that transport
//! has cases for is not applicable. The verdicts read the cases at the lowest SCSI level judged;
//! hosts do not ask which edition a device follows.

use super::case::{Case, Verdict};

/// One thing a use needs, and the cases that test it.
pub struct Need {
    pub what: &'static str,
    /// The sections whose cases test it, iSCSI and NVMe alike.
    sections: &'static [&'static str],
    /// Words one of which the case name holds; empty for every case of the sections.
    names: &'static [&'static str],
    /// Words that set a case aside: situations this use never meets.
    except: &'static [&'static str],
}

/// One use of a block device.
pub struct Use {
    pub name: &'static str,
    /// The commands it needs, in a line.
    pub commands: &'static str,
    needs: &'static [Need],
}

const fn need(
    what: &'static str,
    sections: &'static [&'static str],
    names: &'static [&'static str],
) -> Need {
    Need {
        what,
        sections,
        names,
        except: &[],
    }
}

/// The reservation cases a cluster of hosts meets: every node has an initiator name of its own and
/// releases with the type it reserved, so the cases about ports sharing a name, and about releasing
/// with the wrong type, are set aside.
const CLUSTER_RESERVATIONS: Need = Need {
    what: "persistent reservations between hosts",
    sections: &["pr", "nvme-resv"],
    names: &[],
    except: &[
        "initiator name",
        "two ports of one initiator",
        "RELEASE of type",
        "Release naming type",
    ],
};

const SAME_LUN: Need = need(
    "one logical unit on every path",
    &["identity", "nvme-ident"],
    &[],
);

const PATH_RECOVERY: Need = need(
    "path recovery (task management or Abort)",
    &["tmf", "nvme-admin"],
    &["ABORT TASK", "LOGICAL UNIT RESET", "Abort", "Keep Alive"],
);

pub const USES: [Use; 9] = [
    Use {
        name: "Windows disk online (disk.sys)",
        commands: "INQUIRY (36, 74) and VPD 00h, 80h, 83h, B0h, B1h, B2h; READ CAPACITY (10), (16); REPORT LUNS; TEST UNIT READY; MODE SENSE (6) pages 08h, 0Ah, 1Ch",
        needs: &[
            need(
                "the commands Windows Server 2022 sends",
                &["windows"],
                &["online", "enumeration", "MODE SENSE (6)"],
            ),
            need("the commands every device takes", &["spc"], &[]),
        ],
    },
    Use {
        name: "Windows MPIO, active/active/active",
        commands: "VPD 83h with one unique ID on every path; REPORT TARGET PORT GROUPS (or ANA); LOGICAL UNIT RESET and ABORT TASK (or Abort) for path recovery; a session or association re-established",
        needs: &[
            SAME_LUN,
            need("unique LUN ID and path states", &["windows"], &["MPIO", "MSDSM"]),
            need(
                "path states readable",
                &["mpio", "nvme-ana"],
                &["REPORT TARGET PORT GROUPS", "supported state", "ANA"],
            ),
            PATH_RECOVERY,
            need(
                "a session re-established",
                &["session", "nvme-sess"],
                &["reinstat", "association"],
            ),
        ],
    },
    Use {
        name: "NTFS",
        commands: "READ and WRITE (10), (16); FUA; SYNCHRONIZE CACHE (10); VERIFY; caching mode page",
        needs: &[
            need("reads and writes, FUA, flush", &["io", "nvme-io"], &[]),
            need(
                "write cache and flush as Windows sends them",
                &["windows"],
                &["NTFS", "write cache"],
            ),
        ],
    },
    Use {
        name: "Windows TRIM",
        commands: "VPD B0h, B2h; UNMAP; WRITE SAME (16) with UNMAP; GET LBA STATUS (or Dataset Management)",
        needs: &[
            need("TRIM as Windows sends it", &["windows"], &["TRIM"]),
            need(
                "deallocation",
                &["unmap", "nvme-io"],
                &["UNMAP", "LBP", "GET LBA STATUS", "Dataset Management"],
            ),
        ],
    },
    Use {
        name: "Windows ODX",
        commands: "VPD 8Fh; POPULATE TOKEN; WRITE USING TOKEN; RECEIVE ROD TOKEN INFORMATION",
        needs: &[
            need("token copy, or a clean refusal", &["odx"], &[]),
            need("ODX as Windows sees it", &["windows"], &["ODX"]),
        ],
    },
    Use {
        name: "Windows Failover Clustering and CSV",
        commands: "PERSISTENT RESERVE IN and OUT: REGISTER on every path, RESERVE, PREEMPT, CLEAR, from each node",
        needs: &[
            SAME_LUN,
            CLUSTER_RESERVATIONS,
            need(
                "clustering as Windows sees it",
                &["windows"],
                &["Failover Clustering"],
            ),
        ],
    },
    Use {
        name: "VMware ESXi (VAAI)",
        commands: "COMPARE AND WRITE (ATS); UNMAP; REPORT TARGET PORT GROUPS and ALUA unit attentions (or ANA); VPD 83h; EXTENDED COPY (XCOPY, optional)",
        needs: &[
            SAME_LUN,
            need(
                "atomic test and set, or a clean refusal",
                &["ats", "nvme-io"],
                &["COMPARE AND WRITE", "compare", "Compare"],
            ),
            need(
                "UNMAP",
                &["unmap", "nvme-io"],
                &["UNMAP", "Dataset Management"],
            ),
            need("path states", &["mpio", "nvme-ana"], &[]),
        ],
    },
    Use {
        name: "Linux dm-multipath (ALUA)",
        commands: "VPD 83h; REPORT TARGET PORT GROUPS, ALUA states and unit attentions (or ANA); TEST UNIT READY; LOGICAL UNIT RESET",
        needs: &[
            SAME_LUN,
            need("ALUA or ANA", &["mpio", "nvme-ana"], &[]),
            PATH_RECOVERY,
        ],
    },
    Use {
        name: "Shared-disk cluster file systems (GPFS, GFS2, OCFS2)",
        commands: "every node reads and writes one LUN at once; SCSI-3 fencing: REGISTER, RESERVE type 5 or 7, PREEMPT AND ABORT; the same LUN on every path",
        needs: &[
            SAME_LUN,
            CLUSTER_RESERVATIONS,
            need("reads and writes", &["io", "nvme-io"], &[]),
            need("one node's writes read back by another", &["coherence"], &[]),
        ],
    },
];

/// A use's verdict on one transport.
#[derive(Debug, PartialEq)]
pub enum Fit {
    NotApplicable,
    Ready,
    Warnings(usize),
    AtRisk(usize),
    /// Every tested need passed, but some need had no case.
    Partly,
}

impl Need {
    fn tests(&self, c: &Case) -> bool {
        self.sections.contains(&c.section)
            && (self.names.is_empty() || self.names.iter().any(|w| c.name.contains(w)))
            && !self.except.iter().any(|w| c.name.contains(w))
    }
}

impl Use {
    /// The cases of `cases` that test this use, and the verdict they give.
    pub fn judge<'a>(&self, cases: &[&'a Case]) -> (Fit, Vec<&'a Case>) {
        let mut tested = 0;
        let mut mine: Vec<&Case> = Vec::new();
        for n in self.needs {
            let of: Vec<&Case> = cases.iter().copied().filter(|c| n.tests(c)).collect();
            if !of.is_empty() {
                tested += 1;
            }
            for c in of {
                if !mine.iter().any(|m| std::ptr::eq(*m, c)) {
                    mine.push(c);
                }
            }
        }
        let count = |v| mine.iter().filter(|c| c.verdict() == v).count();
        let fit = match (tested, count(Verdict::Fail), count(Verdict::Warn)) {
            (0, _, _) => Fit::NotApplicable,
            (_, f, _) if f > 0 => Fit::AtRisk(f),
            (_, _, w) if w > 0 => Fit::Warnings(w),
            (t, _, _) if t < self.needs.len() => Fit::Partly,
            _ => Fit::Ready,
        };
        (fit, mine)
    }

    /// The needs no case among `cases` tests.
    pub fn untested(&self, cases: &[&Case]) -> Vec<&'static str> {
        self.needs
            .iter()
            .filter(|n| !cases.iter().any(|c| n.tests(c)))
            .map(|n| n.what)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::case::{case, should};
    use super::*;

    fn by_name(name: &str) -> &'static Use {
        USES.iter().find(|u| u.name == name).unwrap()
    }

    fn c(section: &'static str, name: &str, passed: bool) -> Case {
        case(section, name.into(), "e".into(), "a".into(), passed)
    }

    #[test]
    fn a_failing_need_puts_a_use_at_risk_and_a_missing_one_leaves_it_partly_tested() {
        let ok = c("ats", "lun COMPARE AND WRITE", true);
        let same = c("identity", "lun + other", true);
        let unmap = should(
            "unmap",
            "lun UNMAP one block".into(),
            "e".into(),
            "a".into(),
            false,
        );
        let paths = c("mpio", "lun REPORT TARGET PORT GROUPS", true);
        let esx = by_name("VMware ESXi (VAAI)");
        let all = [&ok, &same, &unmap, &paths];
        assert_eq!(esx.judge(&all).0, Fit::Warnings(1));
        assert_eq!(esx.judge(&[&ok, &same, &paths]).0, Fit::Partly);
        assert_eq!(esx.untested(&[&ok, &same, &paths]), vec!["UNMAP"]);
        let bad = c("ats", "lun COMPARE AND WRITE", false);
        assert_eq!(esx.judge(&[&bad, &same, &paths, &unmap]).0, Fit::AtRisk(1));
        assert_eq!(by_name("Windows ODX").judge(&all).0, Fit::NotApplicable);
    }

    #[test]
    fn a_cluster_never_meets_ports_that_share_an_initiator_name() {
        let shared = c(
            "pr",
            "lun two ports of one initiator are two I_T nexuses",
            false,
        );
        let between = c("pr", "lun type 5: B WRITE while A holds", true);
        let same = c("identity", "lun + other", true);
        let windows = c(
            "windows",
            "lun Failover Clustering: SCSI-3 persistent reservations",
            true,
        );
        let cluster = by_name("Windows Failover Clustering and CSV");
        assert_eq!(
            cluster.judge(&[&shared, &between, &same, &windows]).0,
            Fit::Ready
        );
    }
}
