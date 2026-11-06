use crate::Result;
use std::path::PathBuf;

mod help;
mod units;
mod workload;

use help::{help, help_all};
use units::{duration, finite, integer, size};
pub(crate) use workload::workload;
pub use workload::{Backend, PositionMode, Workload};

#[derive(Debug)]
pub struct Config {
    pub cameras: Option<crate::cctv::Settings>,
    pub fsfiller: Option<crate::fsfiller::Settings>,
    pub files: Vec<PathBuf>,
    pub prefixes: Vec<PathBuf>,
    pub jobs: Vec<Workload>,
    pub range: Option<(u64, u64)>,
    pub duration: Option<f64>,
    pub verify: bool,
    pub recreate: bool,
    pub seed: u64,
    pub backend: Backend,
    pub memory: u64,
    pub interval: f64,
    pub errors: u64,
    pub timeout: f64,
    pub positions_file: Option<PathBuf>,
    pub precondition: Option<Workload>,
    pub expected: Option<(u64, u64)>,
    /// Writes one latency per completed operation (milliseconds, one per line) to this file.
    pub latency_out: Option<PathBuf>,
    /// `--iscsi HOST[:PORT]/IQN`, one per target path.
    pub iscsi: Vec<String>,
    /// `--nvmeof HOST[:PORT]/NQN[/NSID]`, one per NVMe-oF (NVMe/TCP) path.
    pub nvmeof: Vec<String>,
    pub iscsi_initiator: Option<String>,
    /// The second name of `--iscsi-initiator A,B`: the other host `--compliance` uses for the
    /// persistent reservation cases, so two of its ports are two initiators, as two cluster nodes are.
    pub iscsi_second_initiator: Option<String>,
    pub chap_user: Option<String>,
    pub chap_secret_file: Option<PathBuf>,
    /// `--compliance`: test every `--iscsi` and `--nvme` target, and every `-f /dev/...` disk, against
    /// the standards and score it.
    pub compliance: bool,
    /// `--compliance-report FILE`: also write the report as an AsciiDoc document.
    pub compliance_report: Option<PathBuf>,
    /// `--scsilevel N`, `N..M` or `N,M`, repeatable: the SCSI levels (SPC-N with SBC-N) the SCSI cases
    /// are judged at; empty means level 3.
    pub scsi_levels: Vec<crate::compliance::ScsiLevel>,
    /// How a workload over several `--iscsi` paths picks a path for each request.
    pub path_policy: crate::paths::Policy,
    /// `-d N`: print the first N commands sent to the iSCSI paths, with their verify bit.
    pub dump: u64,
}
fn expand_options(args: Vec<String>) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(args.len());
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        if arg.starts_with("--") {
            if let Some((key, value)) = arg.split_once('=') {
                out.push(key.into());
                out.push(value.into());
                continue;
            }
            let takes = matches!(arg.as_str(), "--backend" | "--memory");
            out.push(arg);
            if takes {
                out.push(it.next().ok_or("missing option value")?);
            }
            continue;
        }
        if arg.starts_with('-') && arg.len() > 2 && arg[1..].chars().all(|c| c == 'v' || c == 'V') {
            for c in arg[1..].chars() {
                out.push(format!("-{c}"));
            }
            continue;
        }
        let takes = arg.starts_with('-')
            && arg
                .as_bytes()
                .get(1)
                .is_some_and(|c| b"fFGbctRETSspK".contains(c));
        if takes && arg.len() > 2 {
            out.push(arg[..2].into());
            out.push(arg[2..].into());
        } else {
            out.push(arg);
            if takes {
                out.push(it.next().ok_or("missing option value")?);
            }
        }
    }
    Ok(out)
}
/// `--iscsi-initiator A[,B]`: the name spit logs in with, and optionally a second one.
fn initiators(v: &str) -> Result<(String, Option<String>)> {
    let names: Vec<&str> = v.split(',').map(str::trim).collect();
    match names.as_slice() {
        [a] if !a.is_empty() => Ok((a.to_string(), None)),
        [a, b] if !a.is_empty() && !b.is_empty() && a != b => {
            Ok((a.to_string(), Some(b.to_string())))
        }
        _ => Err("--iscsi-initiator takes IQN or IQN,IQN (two different names)".into()),
    }
}

pub fn parse(args: Vec<String>) -> Result<Option<Config>> {
    let args = expand_options(args)?;
    if let Some(pos) = args.iter().position(|a| a == "-h" || a == "--help") {
        if args.get(pos + 1).is_some_and(|a| a == "all") {
            help_all();
        } else {
            help();
        }
        return Ok(None);
    }
    let mut c = Config {
        cameras: None,
        fsfiller: None,
        files: vec![],
        prefixes: vec![],
        jobs: vec![],
        range: None,
        duration: Some(10.),
        verify: false,
        recreate: false,
        seed: 1,
        backend: Backend::Direct,
        memory: crate::memory::default_budget(),
        interval: 1.,
        errors: 1,
        timeout: 3600.,
        positions_file: None,
        precondition: None,
        expected: None,
        latency_out: None,
        iscsi: Vec::new(),
        nvmeof: Vec::new(),
        iscsi_initiator: None,
        iscsi_second_initiator: None,
        chap_user: None,
        chap_secret_file: None,
        compliance: false,
        compliance_report: None,
        scsi_levels: Vec::new(),
        path_policy: crate::paths::Policy::RoundRobin,
        dump: 0,
    };
    if args.is_empty() {
        help();
        return Err("specify a target and workload".into());
    }
    let mut it = args.iter();
    let mut length = 0;
    let mut explicit_duration = false;
    let mut explicit_backend = false;
    let mut camera_block = None;
    let mut file_size = None;
    let mut cctv_io = None;
    let mut cctv_floor = None;
    let mut cctv_link = None;
    let mut cctv_drop_after = None;
    let mut camera_conflict = false;
    let mut fsfiller = false;
    let mut fsfiller_dirs = None;
    let mut fsfiller_target = None;
    let mut fsfiller_threads = None;
    let mut fsfiller_cant_stop = false;
    let mut fsfiller_sync = false;
    let mut fsfiller_writes = None;
    while let Some(flag) = it.next() {
        camera_conflict |= matches!(
            flag.as_str(),
            "-f" | "-G" | "-b" | "-v" | "--recreate" | "-p" | "-S" | "-K" | "-E" | "-T"
        );
        match flag.as_str() {
            "--version" => {
                println!("spit {} (Rust)", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-v" => {
                c.verify = true;
                continue;
            }
            "--compliance" => {
                c.compliance = true;
                continue;
            }
            "-V" => continue,
            "--recreate" => {
                c.recreate = true;
                continue;
            }
            "--fsfiller" => {
                fsfiller = true;
                continue;
            }
            "--cantstopthis" => {
                fsfiller_cant_stop = true;
                continue;
            }
            "--synconclose" => {
                fsfiller_sync = true;
                continue;
            }
            _ => {}
        }
        let v = it
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "-f" => c.files.push(v.into()),
            "--iscsi" | "--scsi" => c.iscsi.extend(crate::paths::expand_connections(v)?),
            "--nvmeof" | "--nvme" => c.nvmeof.extend(crate::paths::expand_connections(v)?),
            "--compliance-report" => c.compliance_report = Some(v.into()),
            "--scsilevel" => {
                c.scsi_levels
                    .extend(crate::compliance::parse_scsi_levels(v)?);
                c.scsi_levels.sort_unstable();
                c.scsi_levels.dedup();
            }
            "--iscsi-initiator" => {
                let (first, second) = initiators(v)?;
                c.iscsi_initiator = Some(first);
                c.iscsi_second_initiator = second;
            }
            "-d" => c.dump = integer(v)?,
            "--iscsi-policy" | "--policy" => c.path_policy = crate::paths::Policy::parse(v)?,
            "--chap-user" => c.chap_user = Some(v.to_string()),
            "--chap-secret-file" => c.chap_secret_file = Some(v.into()),
            "-F" => c.prefixes.push(v.into()),
            "--writes" => {
                fsfiller_writes = Some(match v.as_str() {
                    "buffered" => crate::fsfiller::WriteMode::Buffered,
                    "writethrough" => crate::fsfiller::WriteMode::WriteThrough,
                    "direct" => crate::fsfiller::WriteMode::Direct,
                    _ => return Err("--writes takes buffered, writethrough or direct".into()),
                });
            }
            "--fsfiller-threads" => {
                let n = integer(v)?;
                if !(1..=256).contains(&n) {
                    return Err("--fsfiller-threads must be from 1 to 256".into());
                }
                fsfiller_threads = Some(n as usize);
            }
            "--ratelimitmbps" => {
                let mbit: f64 = v
                    .parse()
                    .ok()
                    .filter(|m: &f64| m.is_finite() && *m > 0.)
                    .ok_or("--ratelimitmbps needs a positive Mbit/s, e.g. 1000")?;
                fsfiller_target = Some(mbit);
            }
            "--fsfiller-dirs" => {
                let (top, second) = v
                    .split_once(',')
                    .ok_or("--fsfiller-dirs needs TOP,SECOND, e.g. 100,100")?;
                let (top, second) = (integer(top)?, integer(second)?);
                if !(1..=10000).contains(&top) || !(1..=10000).contains(&second) {
                    return Err("--fsfiller-dirs counts must be from 1 to 10000".into());
                }
                fsfiller_dirs = Some((top as usize, second as usize));
            }
            "-c" => {
                camera_block = Some(v.clone());
                length += v.len() + 1;
                if length > 4094 {
                    return Err("command too long".into());
                }
                c.jobs.push(workload(v)?);
            }
            "-G" => {
                c.range = Some(if let Some((a, b)) = v.split_once('-') {
                    (size(a, 1e9)?, size(b, 1e9)?)
                } else {
                    (0, size(v, 1e9)?)
                });
            }
            "-b" => c.range = Some((0, size(v, 1.)?)),
            "-t" => {
                explicit_duration = true;
                c.duration = duration(v)?;
            }
            "-R" => c.seed = integer(v)?,
            "-s" => {
                c.interval = finite(v)?;
                if c.interval != 0. && !(0.001..=86400.).contains(&c.interval) {
                    return Err(
                        "progress interval must be 0 or in [0.001, 86400]; -s 0 disables it".into(),
                    );
                }
            }
            "-E" => c.errors = integer(v)?,
            "-T" => {
                c.timeout = finite(v)?;
                if c.timeout <= 0. || c.timeout > 3600. {
                    return Err("timeout must be in (0, 3600]".into());
                }
            }
            "--backend" => {
                c.backend = Backend::parse(v)?;
                explicit_backend = v != "auto";
            }
            "--cctv" => {
                if c.cameras.is_some() {
                    return Err("specify --cctv once".into());
                }
                c.cameras = Some(crate::cctv::Settings::parse(v)?);
            }
            "--cctv-drop-after" => {
                let seconds: f64 = v
                    .parse()
                    .map_err(|_| "--cctv-drop-after needs seconds, e.g. 5")?;
                if seconds.is_nan() || seconds <= 0. {
                    return Err("--cctv-drop-after must be above 0".into());
                }
                cctv_drop_after = Some(seconds);
            }
            "--link-mbps" => {
                let mbps: f64 = v
                    .parse()
                    .map_err(|_| "--link-mbps needs a rate in Mbit/s, e.g. 1000")?;
                if mbps.is_nan() || mbps <= 0. {
                    return Err("--link-mbps must be above 0".into());
                }
                cctv_link = Some(mbps);
            }
            "--cctv-floor-percent" => {
                let percent: f64 = v
                    .parse()
                    .map_err(|_| "--cctv-floor-percent needs a percentage, e.g. 75")?;
                if !(0. ..=100.).contains(&percent) {
                    return Err(
                        "--cctv-floor-percent must be from 0 to 100; 0 turns the floor off".into(),
                    );
                }
                cctv_floor = Some(percent / 100.);
            }
            "--cctv-io" => {
                cctv_io = Some(match v.as_str() {
                    "pagecache" => false,
                    "direct" => true,
                    _ => return Err("--cctv-io needs pagecache or direct".into()),
                });
            }
            "--filesizemb" => {
                file_size = Some(size(&format!("{v}MiB"), 1.)?);
            }
            "--memory" => c.memory = size(v, 1.)?,
            "--latency-out" => c.latency_out = Some(PathBuf::from(v)),
            "-S" => c.positions_file = Some(v.into()),
            "-p" => c.precondition = Some(workload(&format!("ws1x1{v}"))?),
            "-K" => {
                let (a, b) = v
                    .split_once(',')
                    .ok_or("expected -K read_count,write_count")?;
                c.expected = Some((integer(a)?, integer(b)?));
            }
            _ => return Err(format!("unsupported option {flag}")),
        }
    }
    if fsfiller {
        if c.cameras.is_some() || c.prefixes.len() != 1 || !c.files.is_empty() || !c.jobs.is_empty()
        {
            return Err("--fsfiller takes one -F directory, with optional -G cap, -t limit and --fsfiller-dirs; it cannot be combined with -f, -c or --cctv".into());
        }
        let (top, second) = fsfiller_dirs.unwrap_or((100, 100));
        let ratelimit_mbps = fsfiller_target;
        if fsfiller_writes == Some(crate::fsfiller::WriteMode::Direct)
            && !cfg!(any(windows, target_os = "linux", target_os = "freebsd"))
        {
            return Err("--writes direct is not available on this platform".into());
        }
        c.fsfiller = Some(crate::fsfiller::Settings {
            root: c.prefixes[0].clone(),
            top,
            second,
            cap: c.range.map(|(lo, hi)| hi - lo),
            duration: if explicit_duration { c.duration } else { None },
            ratelimit_mbps,
            threads: fsfiller_threads.unwrap_or(150),
            cant_stop: fsfiller_cant_stop,
            verify: c.verify,
            sync_on_close: fsfiller_sync,
            write_mode: fsfiller_writes.unwrap_or(crate::fsfiller::WriteMode::WriteThrough),
        });
        return Ok(Some(c));
    }
    if fsfiller_dirs.is_some() {
        return Err("--fsfiller-dirs requires --fsfiller".into());
    }
    if fsfiller_target.is_some() {
        return Err("--ratelimitmbps requires --fsfiller".into());
    }
    if fsfiller_cant_stop {
        return Err("--cantstopthis requires --fsfiller".into());
    }
    if fsfiller_sync {
        return Err("--synconclose requires --fsfiller".into());
    }
    if fsfiller_writes.is_some() {
        return Err("--writes requires --fsfiller".into());
    }
    if let Some(cameras) = &mut c.cameras {
        if camera_conflict || c.prefixes.len() != 1 || c.jobs.len() > 1 {
            return Err("--cctv requires one -F directory/prefix; supports -c kSIZE/MSIZE, --filesizemb, -t, -s, -R and --memory; regular workload/range/verification options cannot be combined".into());
        }
        if let Some(block) = camera_block {
            let scale = if block.starts_with('k') {
                1024.
            } else if block.starts_with('M') {
                1048576.
            } else {
                return Err("CCTV -c accepts a fixed block size only, e.g. -c k64".into());
            };
            cameras.block = usize::try_from(size(&block[1..], scale)?)
                .map_err(|_| "CCTV block size overflow")?;
        }
        cameras.file_size = file_size.unwrap_or(16 * 1048576);
        // Direct I/O where the platform supports it, otherwise the page cache.
        cameras.direct = cctv_io.unwrap_or(cfg!(any(windows, target_os = "linux")));
        if let Some(floor) = cctv_floor {
            cameras.floor = floor;
        }
        cameras.link_mbps = cctv_link;
        if let Some(seconds) = cctv_drop_after {
            cameras.drop_after = seconds;
        }
        cameras.validate(c.memory)?;
        if !explicit_duration {
            c.duration = None;
        }
        if explicit_backend && c.backend != Backend::Pagecache {
            return Err(
                "CCTV mode uses page-cache filesystem I/O; use --backend pagecache or auto".into(),
            );
        }
        c.backend = Backend::Pagecache;
        return Ok(Some(c));
    }
    if file_size.is_some() {
        return Err("--filesizemb requires --cctv".into());
    }
    if cctv_io.is_some() {
        return Err("--cctv-io requires --cctv".into());
    }
    if cctv_floor.is_some() {
        return Err("--cctv-floor-percent requires --cctv".into());
    }
    if cctv_link.is_some() {
        return Err("--link-mbps requires --cctv".into());
    }
    if cctv_drop_after.is_some() {
        return Err("--cctv-drop-after requires --cctv".into());
    }
    if !c.iscsi.is_empty() && c.iscsi_initiator.is_none() {
        let (name, from) = crate::iscsi::initiator::default_name();
        println!("# using --iscsi-initiator {name}");
        println!("#   from {from}; give --iscsi-initiator IQN to override");
        c.iscsi_initiator = Some(name);
    }
    let disks = c.files.iter().any(|f| f.starts_with("/dev/"));
    if c.compliance && c.iscsi.is_empty() && c.nvmeof.is_empty() && !disks {
        return Err(
            "--compliance needs --iscsi or --nvme hosts or targets, or -f /dev/... disks".into(),
        );
    }
    // --compliance runs no workload; its -f names are disks to test.
    if c.compliance {
        if c.files.iter().any(|f| !f.starts_with("/dev/")) {
            return Err("--compliance tests -f /dev/... disks, not files".into());
        }
        return Ok(Some(c));
    }
    if !c.nvmeof.is_empty() {
        if !c.files.is_empty() || !c.prefixes.is_empty() || fsfiller || c.cameras.is_some() {
            return Err("--nvmeof cannot be combined with -f, -F, --fsfiller or --cctv".into());
        }
        return Ok(Some(c));
    }
    if !c.iscsi.is_empty() {
        if !c.files.is_empty() || !c.prefixes.is_empty() || fsfiller || c.cameras.is_some() {
            return Err("--iscsi reports on targets; it cannot be combined with -f, -F, --fsfiller or --cctv".into());
        }
        return Ok(Some(c));
    }
    if c.files.is_empty() && c.prefixes.is_empty() {
        return Err("specify -f FILE or -F PREFIX".into());
    }
    if c.jobs.is_empty() {
        return Err("specify -c WORKLOAD".into());
    }
    if let Some((lo, hi)) = c.range {
        if lo >= hi {
            return Err("empty or reversed range".into());
        }
    }
    if c.files
        .len()
        .checked_add(c.prefixes.len())
        .and_then(|n| n.checked_mul(c.jobs.iter().map(|j| j.workers).sum()))
        .filter(|n| *n <= 1024)
        .is_none()
    {
        return Err("too many workers (maximum 1024)".into());
    }
    Ok(Some(c))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compliance_takes_local_disks_and_no_workload() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let c = parse(args(&["-f", "/dev/sdx", "--compliance"]))
            .unwrap()
            .unwrap();
        assert!(c.compliance && c.jobs.is_empty());
        assert_eq!(
            parse(args(&["-f", "/dev/sdx", "-f", "data.bin", "--compliance"])).unwrap_err(),
            "--compliance tests -f /dev/... disks, not files"
        );
    }

    #[test]
    fn duration_suffixes_are_exact_and_bounded() {
        for (text, expected) in [
            ("7d", 604800.),
            ("1.5h", 5400.),
            ("10m", 600.),
            ("15m", 900.),
            ("250ms", 0.25),
            ("2w", 1209600.),
            ("1s", 1.),
            ("0.1", 0.1),
        ] {
            assert_eq!(duration(text).unwrap(), Some(expected));
        }
        assert_eq!(duration("-1").unwrap(), None);
        for text in ["0", "-2", "nan", "1e300d", "1day", "7dextra"] {
            assert!(duration(text).is_err(), "{text}");
        }
    }
    #[test]
    fn reject_unbounded_input() {
        for s in [
            "wq0",
            "wj-1",
            "wq99999999999999999999",
            "wpnan",
            "wk0",
            "wP0",
        ] {
            assert!(workload(s).is_err(), "{s}");
        }
    }
    #[test]
    fn parses_c_syntax() {
        let w = workload("ws1k4q32j64P8X1").unwrap();
        assert_eq!((w.block, w.depth, w.workers), (4096, 32, 64));
        assert_eq!(w.read_probability, 0.);
    }
    #[test]
    fn accepts_the_reported_large_block_workload() {
        assert!(workload("wk4096s0j32G400R42").is_ok());
    }

    #[test]
    fn the_initiator_option_takes_one_or_two_names() {
        assert_eq!(initiators("iqn.a").unwrap(), ("iqn.a".into(), None));
        assert_eq!(
            initiators("iqn.a,iqn.b").unwrap(),
            ("iqn.a".into(), Some("iqn.b".into()))
        );
        assert!(initiators("iqn.a,iqn.a").is_err());
        assert!(initiators("iqn.a,").is_err());
        assert!(initiators("a,b,c").is_err());
    }
}
