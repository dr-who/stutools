//! Microsoft HLK NVMe SCSI Compliance Test (LOGO), as spit runs it: each of the suite's tests by its own ID, the spit cases that
//! decide it, and what spit does not yet run. Built from the suites inventory; a test spit
//! starts running gets its cases and coverage here.

use super::super::suite::{Coverage::*, Ref, Style, Suite, Test};

pub const HLK_NVME_SCSI_COMPLIANCE: Suite = Suite {
    title: "Microsoft HLK NVMe SCSI Compliance Test (LOGO)",
    publisher: "Microsoft",
    source: "https://learn.microsoft.com/en-us/windows-hardware/test/hlk/testref/1f98eed5-478b-42bc-8c17-ee49a2c63202",
    style: Style::Hlk,
    transports: &["NVMe/TCP"],
    tests: &[
        Test::new("NVS-1", "REPORT_LUNS", Covered, &[Ref::new("nvme-ident", "NVMe 1.1 Active Namespace ID list")], ""),
        Test::new("NVS-2", "INQUIRY", Covered, &[Ref::new("nvme-ident", "NVMe 1.0 Identify Controller"), Ref::new("nvme-ident", "NVMe 1.0 Identify Namespace 1")], ""),
        Test::new("NVS-3", "MODE_SENSE / MODE_SENSE10", Covered, &[Ref::new("windows", "write cache: caching mode page WCE (SCSI-NVMe 1.5 6.3.3.2)"), Ref::new("nvme-admin", "NVMe 1.0 Volatile Write Cache feature")], ""),
        Test::new("NVS-4", "MODE_SELECT / MODE_SELECT10", Partial, &[], "Set Features Volatile Write Cache (WCE change) never sent"),
        Test::new("NVS-5", "READ_CAPACITY / READ_CAPACITY16", Covered, &[Ref::new("nvme-ident", "ns 1 NVMe 1.0 NSZE, NCAP, NUSE"), Ref::new("nvme-ident", "ns 1 NVMe 1.0 FLBAS")], ""),
        Test::new("NVS-6", "READ6 / WRITE6", NotApplicable, &[], "translated by stornvme to Read/Write; covered by the Read and Write rows"),
        Test::new("NVS-7", "READ (10) / READ16", Covered, &[Ref::new("nvme-io", "ns 1 NVMe 1.0 Read and Write read back the write"), Ref::new("nvme-io", "read the last block"), Ref::new("nvme-io", "read past the end")], ""),
        Test::new("NVS-8", "WRITE (10) / WRITE16", Covered, &[Ref::new("nvme-io", "ns 1 NVMe 1.0 Read and Write write"), Ref::new("nvme-io", "write past the end")], ""),
        Test::new("NVS-9", "SYNCHRONIZE_CACHE", Covered, &[Ref::new("windows", "NTFS: SYNCHRONIZE CACHE as Flush (SCSI-NVMe 1.5 5.5)"), Ref::new("nvme-io", "ns 1 NVMe 1.0 Flush")], ""),
        Test::new("NVS-10", "UNMAP", Covered, &[Ref::new("windows", "TRIM: UNMAP as Dataset Management deallocate (SCSI-NVMe 1.5 5.6)"), Ref::new("nvme-io", "Dataset Management deallocate")], ""),
        Test::new("NVS-11", "START_STOP_UNIT", NotApplicable, &[], "stornvme answers it itself (power state); no NVMe command to judge on a fabrics target"),
        Test::new("NVS-12", "SECURITY_PROTOCOL_IN / OUT", Missing, &[], "Security Receive / Send (82h/81h) never sent; expect Invalid Command Opcode when OACS bit 0 is clear"),
        Test::new("NVS-13", "LOG_SENSE (00h, 0Dh, 2Fh)", Partial, &[Ref::new("nvme-admin", "NVMe 1.0 SMART / Health log page"), Ref::new("nvme-admin", "SMART / Health contents")], "temperature and critical warning translation not judged"),
        Test::new("NVS-14", "REQUEST_SENSE", NotApplicable, &[], "answered by stornvme"),
        Test::new("NVS-15", "TEST_UNIT_READY", Covered, &[Ref::new("nvme-prop", "enable")], ""),
        Test::new("NVS-16", "VPD 00h / 80h / 83h", Covered, &[Ref::new("windows", "MPIO: Device Identification VPD from the namespace IDs (SCSI-NVMe 1.5 6.1.4)"), Ref::new("nvme-ident", "ns 1 NVMe 1.3 Namespace Identification Descriptor list contents")], ""),
        Test::new("NVS-17", "VPD B0h", Partial, &[Ref::new("nvme-ident", "NVMe 1.0 Identify Controller")], "MDTS read); DMRL/DMRSL (Dataset Management limits) not judged"),
        Test::new("NVS-18", "VPD B1h", NotApplicable, &[], "rotation rate is fixed by the translation (non-rotating)"),
        Test::new("NVS-19", "Mode page 08h Caching", Covered, &[Ref::new("windows", "write cache: caching mode page WCE (SCSI-NVMe 1.5 6.3.3.2)")], ""),
        Test::new("NVS-20", "PERSISTENT RESERVE (Failover Clustering)", Covered, &[Ref::new("windows", "Failover Clustering: PERSISTENT RESERVE as reservations (SCSI-NVMe 1.5 4.13)")], ""),
    ],
};
