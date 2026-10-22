use crate::Result;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    Libaio,
    Uring,
    Buffered,
    Direct,
}
impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Self::Libaio => "libaio",
            Self::Uring => "io_uring",
            Self::Buffered => "buffered",
            Self::Direct => "direct",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "auto" => Ok(crate::io::default_backend()),
            "libaio" | "aio" => Ok(Self::Libaio),
            "io_uring" | "uring" => Ok(Self::Uring),
            "buffered" | "bufferedio" => Ok(Self::Buffered),
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
    pub errors: u64,
    pub timeout: f64,
    pub positions_file: Option<PathBuf>,
    pub precondition: Option<Workload>,
    pub expected: Option<(u64, u64)>,
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
                .is_some_and(|c| b"fFGbctRETS pK".contains(c) && *c != b' ');
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
    let mut c = Config {
        files: vec![],
        prefixes: vec![],
        jobs: vec![],
        range: None,
        duration: Some(10.),
        verify: false,
        recreate: false,
        seed: 1,
        backend: crate::io::default_backend(),
        memory: 512 * 1024 * 1024,
        errors: 1,
        timeout: 1.,
        positions_file: None,
        precondition: None,
        expected: None,
    };
    if args.is_empty() {
        help();
        return Err("specify a target and workload".into());
    }
    let mut it = args.iter();
    let mut length = 0;
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "-h" | "--help" => {
                help();
                return Ok(None);
            }
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
            _ => {}
        }
        let v = it
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "-f" => c.files.push(v.into()),
            "-F" => c.prefixes.push(v.into()),
            "-c" => {
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
                let t = finite(v)?;
                if t <= 0. && t != -1. {
                    return Err("duration must be positive or -1".into());
                }
                c.duration = if t == -1. { None } else { Some(t) };
            }
            "-R" => c.seed = integer(v)?,
            "-E" => c.errors = integer(v)?,
            "-T" => {
                c.timeout = finite(v)?;
                if c.timeout <= 0. || c.timeout > 3600. {
                    return Err("timeout must be in (0, 3600]".into());
                }
            }
            "--backend" => c.backend = Backend::parse(v)?,
            "--memory" => c.memory = size(v, 1.)?,
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
fn help() {
    println!(
        "Default backend on this system: {} (auto)",
        crate::io::default_backend().name()
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
  -G SIZE         bytes [0,SIZE); -G LOW-HIGH selects [LOW,HIGH)
                  bare numbers are decimal GB; KiB/MiB/GiB are exact
  -b BYTES        size in bytes (equivalent to -G with a byte suffix)
  -c WORKLOAD     workload string; repeat to run workloads together
  -t SECONDS      time limit (default 10); -1 unlimited; x/X override it
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
  WON:OFF         active/idle seconds, e.g. W1:0.5
  GLOW-HIGH       per-workload GiB range; bLOW-HIGH is a byte range
                  bare G uses the target range; G_ also splits J workers

VERIFICATION, RESOURCES AND DIAGNOSTICS
  -v              read back each write; verification reads are not workload ops
  -R SEED         repeatable random seed (default 1)
  -S FILE         positions: byte-offset R|W byte-length, one per line
  -p WORKLOAD     precondition first (one worker; defaults to ws1x1)
  -K READ,WRITE   require these completed workload counts
  -E ERRORS       libaio submission-error stop threshold (default 1; 0 continues)
                  any submission error still makes the final exit nonzero
  -T SECONDS      async completion-stall timeout (default 1; synchronous calls
                  and kernel queue destruction cannot guarantee this bound)
  --backend NAME  auto | io_uring | libaio | direct | buffered (bufferedio alias)
  --memory SIZE   buffer/table budget (default 512MiB); excludes stacks,
                  runtime, kernel queues and OS page cache
  -h, --help      this help and the detected backend; --version prints version

Linux auto probes io_uring queue creation, then uses libaio if unavailable.
macOS/Windows use buffered positional I/O. Explicit backends never fall back.
Synchronous backends do one I/O at a time per worker; q sets reusable batch size.
Pools allocate at startup; successful worker I/O loops report hot_path_allocations=0.
Summary: completed reads/writes, elapsed seconds, MiB/s, IOPS and verified_ops.
Metadata checks pending at a limit finish before success, outside workload counts.
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
}
