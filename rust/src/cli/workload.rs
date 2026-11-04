//! Workload syntax: the -c tokens, their backend and position types.
use super::units::{finite, integer, size};
use crate::Result;

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
    /// Bare `v`: each write is followed at once by a read of the same block, on another path.
    pub write_verify: bool,
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
            write_verify: false,
        }
    }
}
pub(crate) fn workload(s: &str) -> Result<Workload> {
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
    let mut write_verify = false;
    let mut explicit_depth = false;
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
                explicit_depth = true;
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
            'v' if value.is_empty() => write_verify = true,
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
    if write_verify {
        // Writes only, each verified at once; a single queue slot so the read follows its write.
        w.read_probability = 0.;
        w.write_verify = true;
        // One slot unless `q` was given: with more, a read can overtake its write, which is the point.
        if !explicit_depth {
            w.depth = 1;
        }
        w.verify = true;
    }
    w.verify = w.verify || w.unique_verify || (w.metadata.is_some() && !w.unique);
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
