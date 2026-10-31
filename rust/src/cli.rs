use crate::Result;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    Libaio,
    Uring,
    Pagecache,
    Direct,
}
impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Self::Libaio => "libaio",
            Self::Uring => "io_uring",
            Self::Pagecache => "pagecache",
            Self::Direct => "direct",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "auto" => Ok(crate::io::default_backend()),
            "libaio" | "aio" => Ok(Self::Libaio),
            "io_uring" | "uring" => Ok(Self::Uring),
            "pagecache" => Ok(Self::Pagecache),
            "direct" => Ok(Self::Direct),
            _ => Err(format!("unknown backend {s}")),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PositionMode {
    First,
    Sample,
    Spaced,
    Alternating,
}
#[derive(Clone, Debug)]
pub struct Workload {
    pub read_probability: f64,
    pub block: usize,
    pub max_block: usize,
    pub power_sizes: bool,
    pub mode: PositionMode,
    pub shuffle: bool,
    pub shift: bool,
    pub from_zero: bool,
    pub depth: usize,
    pub submit_batch: Option<usize>,
    pub operation_limit: Option<u64>,
    pub alternate: bool,
    pub rate: Option<f64>,
    pub duty: Option<(f64, f64)>,
    pub duration: Option<f64>,
    pub seed: Option<u64>,
    pub workers: usize,
    pub shared_range: bool,
    pub range: Option<(u64, u64)>,
    pub split_range: bool,
    pub sequential: f64,
    pub positions: Option<u64>,
    pub passes: Option<u64>,
    pub range_passes: Option<u64>,
    pub verify: bool,
    pub metadata: Option<usize>,
    pub unique: bool,
    pub unique_verify: bool,
    pub flush: bool,
}
impl Default for Workload {
    fn default() -> Self {
        Self {
            read_probability: 0.5,
            block: 4096,
            max_block: 4096,
            power_sizes: false,
            mode: PositionMode::First,
            shuffle: false,
            shift: false,
            from_zero: false,
            depth: 64,
            submit_batch: None,
            operation_limit: None,
            alternate: false,
            rate: None,
            duty: None,
            duration: None,
            seed: None,
            workers: 1,
            shared_range: false,
            range: None,
            split_range: false,
            sequential: 1.,
            positions: None,
            passes: None,
            range_passes: None,
            verify: false,
            metadata: None,
            unique: false,
            unique_verify: false,
            flush: false,
        }
    }
}
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
}
fn finite(s: &str) -> Result<f64> {
    let v = s
        .parse::<f64>()
        .map_err(|_| format!("invalid number {s}"))?;
    if !v.is_finite() {
        return Err(format!("nonfinite number {s}"));
    }
    Ok(v)
}
fn duration(s: &str) -> Result<Option<f64>> {
    if s == "-1" {
        return Ok(None);
    }
    let (number, multiplier) = if let Some(s) = s.strip_suffix("ms") {
        (s, 0.001)
    } else if let Some(s) = s.strip_suffix('s') {
        (s, 1.)
    } else if let Some(s) = s.strip_suffix('m') {
        (s, 60.)
    } else if let Some(s) = s.strip_suffix('h') {
        (s, 3600.)
    } else if let Some(s) = s.strip_suffix('d') {
        (s, 86400.)
    } else if let Some(s) = s.strip_suffix('w') {
        (s, 604800.)
    } else {
        (s, 1.)
    };
    let seconds = finite(number)? * multiplier;
    if seconds <= 0. || std::time::Duration::try_from_secs_f64(seconds).is_err() {
        return Err("duration must be positive (seconds, ms/s/m/h/d/w suffix) or -1".into());
    }
    Ok(Some(seconds))
}
pub fn size(s: &str, default: f64) -> Result<u64> {
    let n = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let unit = match &s[n..] {
        "" => default as u128,
        "B" => 1,
        "k" | "K" | "KB" => 1000,
        "M" | "MB" => 1_000_000,
        "G" | "GB" => 1_000_000_000,
        "T" | "TB" => 1_000_000_000_000,
        "KiB" => 1024,
        "MiB" => 1048576,
        "GiB" => 1073741824,
        "TiB" => 1099511627776,
        _ => return Err(format!("invalid size {s}")),
    };
    let (whole, fraction) = s[..n].split_once('.').unwrap_or((&s[..n], ""));
    if whole.is_empty() && fraction.is_empty() {
        return Err(format!("invalid size {s}"));
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole
            .parse::<u128>()
            .map_err(|_| format!("invalid size {s}"))?
    };
    let scale = 10u128
        .checked_pow(fraction.len() as u32)
        .ok_or("size precision overflow")?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u128>()
            .map_err(|_| format!("invalid size {s}"))?
    };
    let value = whole
        .checked_mul(scale)
        .and_then(|v| v.checked_add(fraction))
        .and_then(|v| v.checked_mul(unit))
        .ok_or("size overflow")?;
    if value % scale != 0 || value / scale > i64::MAX as u128 {
        return Err(format!("invalid size {s}"));
    }
    Ok((value / scale) as u64)
}
fn integer(s: &str) -> Result<u64> {
    s.parse()
        .map_err(|_| format!("invalid unsigned integer {s}"))
}
pub fn workload(s: &str) -> Result<Workload> {
    if !s.is_ascii() {
        return Err("workload syntax must be ASCII".into());
    }
    if s.len() > 4093 {
        return Err("command too long".into());
    }
    let mut w = Workload::default();
    let mut reads = 0;
    let mut writes = 0;
    let mut probability = None;
    let mut explicit_sequential = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let flag = bytes[i] as char;
        i += 1;
        let start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_digit()
                || bytes[i] == b'.'
                || bytes[i] == b'-'
                || bytes[i] == b'+'
                || bytes[i] == b'_'
                || bytes[i] == b':')
        {
            i += 1;
        }
        let value = &s[start..i];
        match flag {
            'r' if value.is_empty() => reads += 1,
            'w' if value.is_empty() => writes += 1,
            'p' => {
                let p = finite(value)?;
                if !(0.0..=1.).contains(&p) {
                    return Err("probability must be 0..1".into());
                }
                probability = Some(p);
            }
            'k' | 'M' => {
                let (low, high) = value.split_once(['-', ':']).unwrap_or((value, value));
                let unit = if flag == 'k' { 1024. } else { 1048576. };
                w.block = usize::try_from(size(low, unit)?).map_err(|_| "block too large")?;
                w.max_block = usize::try_from(size(high, unit)?).map_err(|_| "block too large")?;
                w.power_sizes = value.contains(':');
                if w.block == 0 || w.max_block < w.block || w.max_block > 0x7ffff000 {
                    return Err("invalid block size range".into());
                }
            }
            'q' => {
                let (low, high) = value
                    .split_once('-')
                    .map_or((None, value), |(a, b)| (Some(a), b));
                w.submit_batch = low
                    .map(|n| {
                        integer(n).and_then(|n| {
                            usize::try_from(n).map_err(|_| "submission batch too large".into())
                        })
                    })
                    .transpose()?;
                w.depth = usize::try_from(integer(high)?).map_err(|_| "depth too large")?;
                if w.depth == 0 || w.depth > 4096 {
                    return Err("invalid queue depth".into());
                }
                if w.submit_batch.is_some_and(|n| n == 0 || n > w.depth) {
                    return Err("invalid queue range".into());
                }
            }
            'Y' => {
                let n = integer(value)?;
                w.operation_limit = if n == 0 { None } else { Some(n) };
            }
            'R' => w.seed = Some(integer(value)?),
            'O' if value.is_empty() => w.passes = Some(1),
            '=' if value.is_empty() => w.alternate = true,
            'S' => {
                let rate = finite(value)?;
                if rate <= 0. || rate > 1e12 {
                    return Err("invalid IOPS target".into());
                }
                w.rate = Some(rate);
            }
            'W' => {
                let (on, off) = value.split_once([':', '-']).unwrap_or((value, value));
                let on = finite(on)?;
                let off = finite(off)?;
                if on <= 0. || off < 0. || on + off > 86400. {
                    return Err("invalid duty cycle".into());
                }
                w.duty = Some((on, off));
            }
            'T' | 't' => {
                let t = finite(value)?;
                if t <= 0. || t > 1e9 {
                    return Err("invalid job duration".into());
                }
                w.duration = Some(t);
            }

            'j' | 'J' => {
                w.workers = usize::try_from(integer(value)?).map_err(|_| "workers too large")?;
                w.shared_range = flag == 'J';
            }
            'G' | 'b' => {
                if flag == 'G' && (value.is_empty() || value == "_") {
                    w.range = None;
                    w.split_range = value == "_";
                    continue;
                }
                let (low, high) = value.split_once(['-', '_']).unwrap_or(("0", value));
                let unit = if flag == 'G' { 1073741824. } else { 1. };
                let range = (size(low, unit)?, size(high, unit)?);
                if range.0 >= range.1 {
                    return Err("empty or reversed workload range".into());
                }
                w.range = Some(range);
                w.split_range = value.contains('_');
            }
            's' => {
                explicit_sequential = true;
                w.sequential = finite(value)?;
                if w.sequential.abs() > 65535.
                    || (w.sequential.abs() > 1. && w.sequential.fract() != 0.)
                {
                    return Err("invalid sequential region count".into());
                }
            }
            'P' => {
                let (mode, count) = match value.as_bytes().first() {
                    Some(b'-') => (PositionMode::Sample, &value[1..]),
                    Some(b'+') => (PositionMode::Spaced, &value[1..]),
                    Some(b'.') => (PositionMode::Alternating, &value[1..]),
                    _ => (PositionMode::First, value),
                };
                w.mode = mode;
                w.positions = Some(integer(count)?);
            }
            'n' if value.is_empty() => w.shuffle = true,
            'N' if value.is_empty() => w.shift = true,
            'X' => w.passes = Some(integer(value)?),
            'x' => w.range_passes = Some(integer(value)?),
            'm' => {
                let limit = if value.is_empty() {
                    0
                } else {
                    value
                        .parse::<i128>()
                        .map_err(|_| "invalid metadata count")?
                };
                w.metadata = Some(if limit <= 0 {
                    usize::MAX
                } else {
                    usize::try_from(limit).map_err(|_| "metadata count too large")?
                });
            }
            'u' | 'U' if value.is_empty() => {
                w.unique = true;
                w.unique_verify |= flag == 'U';
            }
            'z' if value.is_empty() => w.from_zero = true,
            'F' if value.is_empty() => w.flush = true,
            _ => return Err(format!("unsupported workload token {flag}{value}")),
        }
    }
    w.read_probability = probability.unwrap_or(if reads + writes == 0 {
        0.5
    } else {
        reads as f64 / (reads + writes) as f64
    });
    w.verify = w.unique_verify || (w.metadata.is_some() && !w.unique);
    if w.metadata.is_some() || w.unique {
        if w.metadata.is_some() {
            w.flush = true;
        }
        if !explicit_sequential && w.metadata.is_some() {
            w.sequential = 0.;
        }
        if let Some(limit) = w.metadata {
            w.depth = w.depth.min(limit);
        }
        if w.unique {
            w.depth = 1;
        }
        w.submit_batch = w.submit_batch.map(|n| n.min(w.depth));
    }
    if w.verify && w.alternate {
        return Err("metadata verification cannot be combined with alternating actions".into());
    }
    if w.depth == 0
        || w.depth > 4096
        || w.workers == 0
        || w.workers > 1024
        || w.block == 0
        || w.block > 0x7ffff000
        || w.positions == Some(0)
        || w.passes == Some(0)
        || w.range_passes == Some(0)
    {
        return Err("invalid workload resource limit".into());
    }
    if w.duration.is_some() && (w.passes.is_some() || w.range_passes.is_some()) {
        return Err("mixing T and x/X is not supported".into());
    }
    if w.passes.is_some() && w.range_passes.is_some() {
        return Err("mixing x and X is not supported".into());
    }
    Ok(w)
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
/// The short overview printed by -h and by a bare run.
fn help() {
    println!("spit {} - Stu's I/O tester", env!("CARGO_PKG_VERSION"));
    println!(
        "Default backend: {} (--backend auto selects {} on this system)",
        Backend::Direct.name(),
        crate::io::default_backend().name()
    );
    println!(
        r#"
Command mode (-c workloads against a target, repeat -c to run several):
  spit -f scratch.img -c rs0k4q32j4 -t 5
  spit -F /mnt/test/f -G 1GiB -c ws1k4q32 -c rs0k4q32 -t 10

App modes (each prints its report when it ends, or on Ctrl-C):
  spit -F /mnt/cctv --cctv 100,5,24       100 cameras at 5 Mbit/s, kept 24 h
  spit --fsfiller -F /mnt/fs -G 50GiB     fill a file system with mixed file sizes

More help:
  spit --help all      every option, workload token and backend
  spit --version       print the version"#
    );
}
fn help_all() {
    println!(
        "Default backend: {} (--backend auto selects {} on this system)",
        Backend::Direct.name(),
        crate::io::default_backend().name()
    );
    println!(
        "Default --memory on this system: {:.3} GiB (automatic)",
        crate::memory::default_budget() as f64 / 1073741824.
    );
    println!(
        r#"spit {} (Rust) - Stu's powerful I/O tester

USAGE
  spit -f TARGET -c WORKLOAD [options]

START HERE (writes overwrite the selected range)
  spit -f scratch.img -G 16MiB -c ws1k4q32j4x1 -v
  spit -f scratch.img -c rs0k4q32j4 -t 5
  spit -f scratch.img -c rws0k4q32j8 -t 10 --backend libaio

  ws1k4q32j4x1 = writes, sequential, 4 KiB, depth 32, 4 workers,
                  one worker-range worth of bytes per worker.

TARGETS AND RUN LIMITS
  -f FILE         file/device; repeat for several targets
  -F PREFIX       one file per worker: PREFIX.0001, PREFIX.0002, ...
                  initialize new test files with -c ws1x1 before reading
  -G SIZE         bytes [0,SIZE); -G LOW-HIGH selects [LOW,HIGH)
                  bare numbers are decimal GB; KiB/MiB/GiB are exact
  -b BYTES        size in bytes (equivalent to -G with a byte suffix)
  -c WORKLOAD     workload string; repeat to run workloads together
  -t SECONDS      measured time limit (default 10); -1 unlimited; x/X override it
                  suffixes ms/s/m/h/d/w accepted, e.g. -t 7d
                  progress/counts stop at this boundary; queued I/O drains silently
  -s SECONDS      live progress interval (default 1); -s 0 disables progress
                  C layout: time / workers / in-flight, read/write MB/s and IOPS,
                  average bytes per I/O, cumulative GiB
  --recreate      permit enlargement of an existing writable regular file

WORKLOAD TOKENS (whitespace allowed inside a quoted workload)
  r / w / rw      reads / writes / 50:50 mix; rrw means 2:1 reads:writes
  pPROB           read probability from 0 to 1 (p0.7 = 70% reads)
  s0 / s1 / s-1  random / sequential / reverse; s2 interleaves 2 stripes
  kN / MN         block size in KiB / MiB (default k4)
  kMIN-MAX        variable blocks in MIN-KiB steps; kMIN:MAX powers of 2
  qN / qMIN-MAX   queue depth / submission-group limit and depth (default q64)
  jN / JN         workers splitting the range / workers sharing the range
  Pn / P-n        first n positions / n random samples with replacement
  P+n / P.n       n spaced positions / alternating low-high positions
  Xn / xn / Yn    n table passes / n range-byte passes / n operations
  O / = / n / N   one table pass / alternate actions / shuffle / shift per pass
  z / F           start at zero / flush completed write batches
  m[COUNT]        metadata write/read checks; optional queue-depth cap
  u / U           per-position seeds / seeds plus checked read pairs (depth 1)
  SIOPS / TSEC    per-worker IOPS limit / duration (not the global -T timeout)
  RSEED           per-workload seed, overriding global -R
  WON:OFF         active/idle seconds, e.g. W1:0.5
  GLOW-HIGH       per-workload GiB range; bLOW-HIGH is a byte range
                  bare G uses the target range; G_ also splits J workers

APPLICATION MODES (instead of -c)
  --cctv COUNT,MBPS,HOURS -F PREFIX   open-loop camera simulation; see [cctv] above
  --fsfiller -F DIR           fill DIR with directories and files of 1 KiB to 16 MiB
                  --fsfiller-dirs TOP,SECOND  directory counts (default 100,100)
                  --fsfiller-threads N  parallel writers (default 150, 1 to 256)
                  --ratelimitmbps N  limit the fill to N Mbit/s, the same unit as --cctv
                  --cantstopthis  retry failed writes, including a full disk, with exponential back-off up to 10 min
                  -v  read each file back after writing and check it
                  --synconclose  flush each file to the device before closing it
                  --writes MODE  buffered, writethrough (default: each write reaches the device before it returns) or direct (bypasses the cache, like O_DIRECT)
                  -G SIZE caps the bytes written; -t SECONDS sets a time limit
                  runs until the file system is full, then reports; Ctrl-C reports early

VERIFICATION, RESOURCES AND DIAGNOSTICS
  -v              read back each write; verification reads are not workload ops
  -R SEED         repeatable random seed (default 1)
  -S FILE         positions: byte-offset R|W byte-length, one per line
  -p WORKLOAD     precondition first (one worker; defaults to ws1x1)
  -K READ,WRITE   require these completed workload counts, including drained I/O
  -E ERRORS       libaio submission-error stop threshold (default 1; 0 continues)
                  any submission error still makes the final exit nonzero
  -T SECONDS      async completion-stall timeout (default 3600, as in spitc; synchronous calls
                  and kernel queue destruction cannot guarantee this bound)
  --backend NAME  auto | io_uring | libaio | direct | pagecache
  --memory SIZE   buffer/table budget (automatic default shown above); excludes stacks,
                  runtime, kernel queues and OS page cache
  -h, --help      short help; --help all for everything; --version prints version

Linux auto probes io_uring queue creation, then uses libaio if unavailable.
Linux administrator enable command:
  sudo sh -c 'echo 0 > /proc/sys/kernel/io_uring_disabled'
macOS/Windows use page-cache positional I/O unless direct is requested. Explicit backends never fall back.
Windows: -f E:\ is a filesystem root; use -F E:\spit-test for file tests.
  spit -F E:\spit-test -G 1MiB -c ws1x1 -v
  spit -F E:\spit-test -G 1MiB -c rs0 -t 10
Administrator raw volume READS: spit -f \\.\C: -c rs0 -G 1GiB
Raw Windows writes are unsupported. Virtual filesystems may have no raw device.
-G limits the range; it never shrinks an existing file. If an I/O stalls beyond
-t, its late completion is excluded from IOPS; the driver must finish before exit.
Synchronous backends do one I/O at a time per worker; q sets reusable batch size.
Pools allocate at startup; successful worker I/O loops report hot_path_allocations=0.
Summary: completed reads/writes, elapsed seconds, MiB/s, IOPS and verified_ops.
Timed summaries count only completions observed within the measured window;
draining, verification cleanup and resource teardown do not extend that window.
Metadata checks pending at a limit finish before success, outside workload counts.
[cctv]
  --cctv COUNT,MBPS,HOURS  camera count (N or START..END/STEP, e.g. 10..100/10),
                           decimal Mbit/s EACH, retention hours
  --info DEVICE           describe a block device (identity, queue and transfer limits,
                          iSCSI negotiated values where the OS exposes them); no I/O
  --cctv-io MODE           direct (default on Linux/Windows) or pagecache
  --link-mbps RATE         link capacity in Mbit/s; drawn on the rate chart
  --cctv-floor-percent PERCENT     stop when the last 10 s average falls below PERCENT of the camera
                           rate (default 0, off); e.g. --cctv-floor-percent 75
  -F DIR/PREFIX           one independent sequential recording stream per camera
  -c k64                  fixed write block size (default 64 KiB; M sizes accepted)
  --filesizemb 16         preallocated recording file size in MiB (default 16)
                          files rotate when full; background worker prepares ahead
  -t 7d                   run for seven days (CCTV default: until Ctrl+C)
  -s SECONDS              normal progress interval (default 1; 0 disables it)
  spit -F recordings/camera --cctv 100,5,24 -c k64 --filesizemb 16 -t 7d
  Logs: cctv-test-UTC-DATETIME-cameras=N.log, .min and .hour, beside the recording prefix.
  A camera range writes one set of logs per count.
  .min/.hour are flushed CSV, with average/max write ms for each minute/hour.
  A completed seven-day run has 168 hourly data rows; final partial periods are labelled.
  Retention deletes only this prefix's owned recording files; logs are preserved.
  CCTV uses direct (no page cache) filesystem I/O on Linux and Windows by default;
  --cctv-io pagecache selects the page cache. Other systems use pagecache I/O.
C data/spitchecker formats and some advanced C modes are not supported; use spitc
for those workflows. See README.md and man spit. Invalid/incomplete I/O fails.

"Premature optimization is the root of all evil." - Donald Knuth
"There is no Rule 6." - Rob Pike
Bring back Pluto as a planet."#,
        env!("CARGO_PKG_VERSION")
    );
}
#[cfg(test)]
mod tests {
    use super::*;
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
}
