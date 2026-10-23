//! `spit --info DEVICE`: describes a block device. Linux sends read-only SCSI commands
//! (INQUIRY, VPD pages, READ CAPACITY, MODE SENSE) and reads the open-iscsi session report.
//! Windows queries the storage device and adapter descriptors. macOS reports `diskutil info`.
//! None of these exposes the iSCSI MaxCmdSN; that value only appears in the login exchange.

use crate::Result;

pub fn run(device: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    return linux::report(device);
    #[cfg(target_os = "windows")]
    return windows::report(device);
    #[cfg(target_os = "macos")]
    return macos::report(device);
    #[allow(unreachable_code)]
    Err(format!("--info is not supported on this platform (device {device})"))
}

#[cfg(target_os = "linux")]
mod linux {
    use super::Result;
    use std::fs::{self, File};
    use std::path::Path;
    use std::process::Command;

    /// Linux SG_IO: sends one SCSI command and returns its status, sense data and any data read.
    mod sg {
        use std::fs::File;
        use std::os::unix::io::AsRawFd;

        const SG_IO: libc::c_ulong = 0x2285;
        const SG_DXFER_NONE: i32 = -1;
        const SG_DXFER_TO_DEV: i32 = -2;
        const SG_DXFER_FROM_DEV: i32 = -3;

        #[repr(C)]
        struct SgIoHdr {
            interface_id: i32,
            dxfer_direction: i32,
            cmd_len: u8,
            mx_sb_len: u8,
            iovec_count: u16,
            dxfer_len: u32,
            dxferp: *mut libc::c_void,
            cmdp: *mut u8,
            sbp: *mut u8,
            timeout: u32,
            flags: u32,
            pack_id: i32,
            usr_ptr: *mut libc::c_void,
            status: u8,
            masked_status: u8,
            msg_status: u8,
            sb_len_wr: u8,
            host_status: u16,
            driver_status: u16,
            resid: i32,
            duration: u32,
            info: u32,
        }

        pub struct Reply {
            pub status: u8,
            pub host_status: u16,
            pub driver_status: u16,
            pub sense: Vec<u8>,
            /// Bytes the device did not transfer.
            pub resid: i32,
        }

        /// Sends `cdb`. With `data` non-empty, the command transfers into it (`to_device` false)
        /// or out of it (`to_device` true).
        pub fn send(file: &File, cdb: &[u8], data: &mut [u8], to_device: bool) -> std::result::Result<Reply, String> {
            let mut sense = [0u8; 32];
            let direction = if data.is_empty() {
                SG_DXFER_NONE
            } else if to_device {
                SG_DXFER_TO_DEV
            } else {
                SG_DXFER_FROM_DEV
            };
            let mut hdr = SgIoHdr {
                interface_id: b'S' as i32,
                dxfer_direction: direction,
                cmd_len: cdb.len() as u8,
                mx_sb_len: sense.len() as u8,
                iovec_count: 0,
                dxfer_len: data.len() as u32,
                dxferp: if data.is_empty() { std::ptr::null_mut() } else { data.as_mut_ptr() as *mut libc::c_void },
                cmdp: cdb.as_ptr() as *mut u8,
                sbp: sense.as_mut_ptr(),
                timeout: 10_000,
                flags: 0,
                pack_id: 0,
                usr_ptr: std::ptr::null_mut(),
                status: 0,
                masked_status: 0,
                msg_status: 0,
                sb_len_wr: 0,
                host_status: 0,
                driver_status: 0,
                resid: 0,
                duration: 0,
                info: 0,
            };
            let rc = unsafe { libc::ioctl(file.as_raw_fd(), SG_IO as _, &mut hdr as *mut SgIoHdr) };
            if rc < 0 {
                return Err(format!("SG_IO: {}", std::io::Error::last_os_error()));
            }
            Ok(Reply {
                status: hdr.status,
                host_status: hdr.host_status,
                driver_status: hdr.driver_status,
                sense: sense[..hdr.sb_len_wr as usize].to_vec(),
                resid: hdr.resid,
            })
        }
    }

    fn sysfs(name: &str, attr: &str) -> String {
        fs::read_to_string(format!("/sys/block/{name}/{attr}"))
            .or_else(|_| fs::read_to_string(format!("/sys/block/{name}/device/{attr}")))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "-".into())
    }

    /// Sends one command and returns the data it read, or a one-line reason it failed.
    fn read_command(file: &File, cdb: &[u8], len: usize) -> std::result::Result<Vec<u8>, String> {
        let mut data = vec![0u8; len];
        let reply = sg::send(file, cdb, &mut data, false)?;
        if reply.status != 0 || reply.host_status != 0 || reply.driver_status != 0 {
            return Err(format!(
                "status=0x{:02x} host=0x{:04x} driver=0x{:04x} sense={:02x?}",
                reply.status, reply.host_status, reply.driver_status, reply.sense
            ));
        }
        let got = len.saturating_sub(reply.resid.max(0) as usize);
        data.truncate(got);
        Ok(data)
    }

    fn ascii(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).trim().to_string()
    }

    pub fn report(device: &str) -> Result<()> {
        let real = fs::canonicalize(device).map_err(|e| format!("{device}: {e}"))?;
        let name = real
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("{device}: not a block device name"))?
            .to_string();
        if !Path::new(&format!("/sys/block/{name}")).exists() {
            return Err(format!("{device} ({name}) is not a block device known to sysfs"));
        }
        println!("device={device} kernel={name}");
        let size = sysfs(&name, "size").parse::<u64>().unwrap_or(0) * 512;
        println!("size_bytes={size}");
        println!(
            "sysfs vendor={} model={} revision={} type={}",
            sysfs(&name, "device/vendor"),
            sysfs(&name, "device/model"),
            sysfs(&name, "device/rev"),
            sysfs(&name, "device/type"),
        );
        println!(
            "queue queue_depth={} nr_requests={} max_sectors_kb={} max_hw_sectors_kb={} scheduler={}",
            sysfs(&name, "device/queue_depth"),
            sysfs(&name, "queue/nr_requests"),
            sysfs(&name, "queue/max_sectors_kb"),
            sysfs(&name, "queue/max_hw_sectors_kb"),
            sysfs(&name, "queue/scheduler"),
        );

        let file = File::open(&real).map_err(|e| format!("{device}: {e}"))?;
        scsi_report(&file);

        match iscsi_session(&name) {
            Some(lines) => {
                println!("iscsi session (open-iscsi, negotiated):");
                for line in lines {
                    println!("  {line}");
                }
            }
            None => println!("iscsi session: none found for {name} (local or non-iSCSI device)"),
        }
        println!("MaxCmdSN: not reported by sysfs, iscsiadm or SCSI; needs an iSCSI login capture");
        Ok(())
    }

    /// Read-only SCSI commands. Each one that fails prints its status and sense data.
    fn scsi_report(file: &File) {
        match read_command(file, &[0x00, 0, 0, 0, 0, 0], 0) {
            Ok(_) => println!("scsi TEST UNIT READY: ok"),
            Err(e) => println!("scsi TEST UNIT READY: {e}"),
        }
        match read_command(file, &[0x12, 0, 0, 0, 36, 0], 36) {
            Ok(d) if d.len() >= 36 => println!(
                "scsi INQUIRY: peripheral=0x{:02x} vendor={} product={} revision={} removable={} command_queue={}",
                d[0] & 0x1f,
                ascii(&d[8..16]),
                ascii(&d[16..32]),
                ascii(&d[32..36]),
                d[1] & 0x80 != 0,
                d[7] & 0x02 != 0,
            ),
            Ok(d) => println!("scsi INQUIRY: short reply of {} bytes", d.len()),
            Err(e) => println!("scsi INQUIRY: {e}"),
        }
        let supported = read_command(file, &[0x12, 0x01, 0x00, 0, 255, 0], 255);
        match &supported {
            Ok(d) if d.len() >= 4 => println!("scsi VPD supported pages: {:02x?}", &d[4..4 + d[3] as usize]),
            Ok(_) => println!("scsi VPD supported pages: short reply"),
            Err(e) => println!("scsi VPD supported pages: {e}"),
        }
        let block_size = match read_command(file, &[0x9e, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0], 32) {
            Ok(d) if d.len() >= 12 => {
                let last = u64::from_be_bytes(d[0..8].try_into().unwrap());
                let block = u32::from_be_bytes(d[8..12].try_into().unwrap());
                println!(
                    "scsi READ CAPACITY(16): blocks={} block_bytes={block} capacity_bytes={}",
                    last + 1,
                    (last + 1) * block as u64
                );
                block as u64
            }
            Ok(_) => {
                println!("scsi READ CAPACITY(16): short reply");
                512
            }
            Err(e) => {
                println!("scsi READ CAPACITY(16): {e}");
                512
            }
        };
        match read_command(file, &[0x12, 0x01, 0x80, 0, 255, 0], 255) {
            Ok(d) if d.len() >= 4 => println!("scsi serial number: {}", ascii(&d[4..4 + d[3] as usize])),
            Ok(_) => println!("scsi serial number: short reply"),
            Err(e) => println!("scsi serial number: {e}"),
        }
        match read_command(file, &[0x12, 0x01, 0x83, 0, 255, 0], 255) {
            Ok(d) if d.len() >= 4 => {
                let end = (4 + u16::from_be_bytes([d[2], d[3]]) as usize).min(d.len());
                let mut at = 4;
                while at + 4 <= end {
                    let kind = d[at + 1] & 0x0f;
                    let len = d[at + 3] as usize;
                    let kind_name = match kind {
                        1 => "T10 vendor",
                        2 => "EUI-64",
                        3 => "NAA",
                        4 => "relative port",
                        8 => "SCSI name",
                        _ => "other",
                    };
                    let value: String = d[at + 4..(at + 4 + len).min(end)].iter().map(|b| format!("{b:02x}")).collect();
                    println!("scsi device identifier: type={kind_name} value={value}");
                    at += 4 + len;
                }
            }
            Ok(_) => println!("scsi device identifiers: short reply"),
            Err(e) => println!("scsi device identifiers: {e}"),
        }
        match read_command(file, &[0x12, 0x01, 0xb0, 0, 255, 0], 64) {
            Ok(d) if d.len() >= 16 => {
                let max_transfer = u32::from_be_bytes(d[8..12].try_into().unwrap());
                let optimal = u32::from_be_bytes(d[12..16].try_into().unwrap());
                println!(
                    "scsi block limits: max_transfer_blocks={max_transfer} ({} bytes) optimal_transfer_blocks={optimal} ({} bytes)",
                    max_transfer as u64 * block_size,
                    optimal as u64 * block_size,
                );
            }
            Ok(_) => println!("scsi block limits: short reply"),
            Err(e) => println!("scsi block limits: {e}"),
        }
        match read_command(file, &[0x12, 0x01, 0xb1, 0, 255, 0], 64) {
            Ok(d) if d.len() >= 6 => {
                let rate = u16::from_be_bytes([d[4], d[5]]);
                let kind = match rate {
                    0 => "not reported".to_string(),
                    1 => "non-rotating (SSD)".to_string(),
                    r => format!("{r} rpm"),
                };
                println!("scsi medium rotation rate: {kind}");
            }
            Ok(_) => println!("scsi medium rotation rate: short reply"),
            Err(e) => println!("scsi medium rotation rate: {e}"),
        }
        match read_command(file, &[0x1a, 0, 0x08, 0, 0x20, 0], 0x20) {
            Ok(d) if d.len() >= 6 && d[4] & 0x3f == 0x08 => {
                println!("scsi write cache: {}", if d[6 + 2] & 0x04 != 0 { "enabled (WCE)" } else { "disabled" })
            }
            Ok(_) => println!("scsi write cache: caching page not found"),
            Err(e) => println!("scsi write cache: {e}"),
        }
    }

    /// Finds the open-iscsi session that owns `name` and returns its target, portal, SID and
    /// negotiated parameters. Each session in `iscsiadm -P 3` starts at its "Current Portal" line.
    fn iscsi_session(name: &str) -> Option<Vec<String>> {
        let out = Command::new("iscsiadm").args(["-m", "session", "-P", "3"]).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let mut target = String::new();
        let mut sessions: Vec<(String, Vec<String>)> = Vec::new();
        let mut in_negotiated = false;
        for line in text.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("Target: ") {
                target = rest.to_string();
            }
            if t.starts_with("Current Portal:") {
                sessions.push((t.to_string(), vec![format!("Target: {target}"), t.to_string()]));
                in_negotiated = false;
            }
            let Some((_, lines)) = sessions.last_mut() else { continue };
            if lines.len() == 2 && t.starts_with("SID:") {
                lines.push(t.to_string());
            }
            if t.starts_with("Negotiated iSCSI params") {
                in_negotiated = true;
            } else if t.starts_with("Attached SCSI devices") {
                in_negotiated = false;
            } else if in_negotiated && t.contains(':') && !t.starts_with('*') {
                lines.push(t.to_string());
            }
        }
        let (_, mut lines) = sessions.into_iter().find(|(portal, _)| attached(&text, portal, name))?;
        // The same parameter can appear once per interface; keep the first copy of each line.
        let mut seen = std::collections::HashSet::new();
        lines.retain(|l| seen.insert(l.clone()));
        Some(lines)
    }

    /// True when the session whose portal line is `portal` lists `name` as an attached disk.
    fn attached(text: &str, portal: &str, name: &str) -> bool {
        let mut inside = false;
        for line in text.lines() {
            let t = line.trim();
            if t == portal {
                inside = true;
            } else if t.starts_with("Current Portal:") {
                inside = false;
            }
            if inside && t.starts_with("Attached scsi disk") {
                return t.split_whitespace().nth(3).is_some_and(|d| d == name);
            }
        }
        false
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::Result;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;

    unsafe extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn DeviceIoControl(
            device: *mut c_void,
            code: u32,
            input: *const c_void,
            input_len: u32,
            output: *mut c_void,
            output_len: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
    }

    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D1400;
    const STORAGE_DEVICE_PROPERTY: u32 = 0;
    const STORAGE_ADAPTER_PROPERTY: u32 = 1;
    const PROPERTY_STANDARD_QUERY: u32 = 0;
    const BUS_TYPES: [&str; 18] = [
        "unknown", "scsi", "atapi", "ata", "1394", "ssa", "fibre", "usb", "raid", "iscsi", "sas", "sata",
        "sd", "mmc", "virtual", "file-backed", "spaces", "nvme",
    ];

    fn query(handle: *mut c_void, property: u32, buffer: &mut [u8]) -> std::result::Result<usize, String> {
        // STORAGE_PROPERTY_QUERY: PropertyId, QueryType, AdditionalParameters[1]
        let mut input = [0u8; 12];
        input[..4].copy_from_slice(&property.to_le_bytes());
        input[4..8].copy_from_slice(&PROPERTY_STANDARD_QUERY.to_le_bytes());
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                input.as_ptr() as *const c_void,
                input.len() as u32,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(format!("IOCTL_STORAGE_QUERY_PROPERTY: {}", std::io::Error::last_os_error()));
        }
        Ok(returned as usize)
    }

    fn text(buffer: &[u8], offset: usize) -> String {
        if offset == 0 || offset >= buffer.len() {
            return "-".into();
        }
        let end = buffer[offset..].iter().position(|&b| b == 0).map_or(buffer.len(), |p| offset + p);
        String::from_utf8_lossy(&buffer[offset..end]).trim().to_string()
    }

    fn u32_at(buffer: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap())
    }

    pub fn report(device: &str) -> Result<()> {
        let wide: Vec<u16> = std::ffi::OsStr::new(device).encode_wide().chain(Some(0)).collect();
        // Query-only access (0) needs no read or write rights to the disk.
        let handle = unsafe {
            CreateFileW(wide.as_ptr(), 0, 3, std::ptr::null_mut(), 3, 0, std::ptr::null_mut())
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("{device}: {}", std::io::Error::last_os_error()));
        }
        // Keep the handle owned so it closes on every path.
        let _owned = unsafe { std::fs::File::from_raw_handle(handle as _) };

        println!("device={device}");
        let mut desc = vec![0u8; 1024];
        query(handle, STORAGE_DEVICE_PROPERTY, &mut desc).map_err(|e| format!("{device}: {e}"))?;
        // STORAGE_DEVICE_DESCRIPTOR: Version, Size, DeviceType, DeviceTypeModifier, RemovableMedia,
        // CommandQueueing, VendorIdOffset, ProductIdOffset, ProductRevisionOffset, SerialNumberOffset,
        // BusType, RawPropertiesLength.
        let bus = u32_at(&desc, 28) as usize;
        println!(
            "scsi vendor={} model={} revision={} serial={}",
            text(&desc, u32_at(&desc, 12) as usize),
            text(&desc, u32_at(&desc, 16) as usize),
            text(&desc, u32_at(&desc, 20) as usize),
            text(&desc, u32_at(&desc, 24) as usize),
        );
        println!(
            "bus={} command_queueing={}",
            BUS_TYPES.get(bus).copied().unwrap_or("other"),
            desc[9] != 0,
        );

        let mut adapter = vec![0u8; 256];
        query(handle, STORAGE_ADAPTER_PROPERTY, &mut adapter).map_err(|e| format!("{device}: {e}"))?;
        // STORAGE_ADAPTER_DESCRIPTOR: Version, Size, MaximumTransferLength, MaximumPhysicalPages,
        // AlignmentMask, AdapterUsesPio, AdapterScansDown, CommandQueueing, AcceleratedTransfer, BusType.
        println!(
            "adapter max_transfer_bytes={} max_physical_pages={} alignment_mask={} command_queueing={}",
            u32_at(&adapter, 8),
            u32_at(&adapter, 12),
            u32_at(&adapter, 16),
            adapter[22] != 0,
        );
        println!("scsi commands: not sent on Windows yet (IOCTL_SCSI_PASS_THROUGH_DIRECT is next)");
        println!("iscsi session: negotiated values are not exposed by Windows WMI; use a login capture");
        println!("MaxCmdSN: not reported by Windows; needs an iSCSI login capture");
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::Result;
    use std::process::Command;

    pub fn report(device: &str) -> Result<()> {
        let out = Command::new("diskutil")
            .args(["info", device])
            .output()
            .map_err(|e| format!("diskutil: {e}"))?;
        if !out.status.success() {
            return Err(format!("diskutil info {device} failed"));
        }
        print!("{}", String::from_utf8_lossy(&out.stdout));
        println!("scsi commands: not sent on macOS (no SCSI pass-through in this build)");
        println!("iscsi session: macOS has no open-iscsi equivalent; negotiated values need a login capture");
        println!("MaxCmdSN: not reported by macOS; needs an iSCSI login capture");
        Ok(())
    }
}
