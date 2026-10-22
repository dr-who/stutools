use crate::{
    cli::{PositionMode, Workload},
    io::Request,
    pattern, Result,
};

// Allocate the entire position table before the timed loop. The capacity never
// grows while running; shuffling is in-place and passes reuse the same table.
pub struct Positions {
    entries: Vec<Request>,
    rng: u64,
    pass: u64,
    lo: u64,
    hi: u64,
    work: Workload,
    pending_checks: usize,
}
impl Positions {
    pub fn new(
        work: &Workload,
        lo: u64,
        hi: u64,
        seed: u64,
        capacity: usize,
        loaded: &[Request],
    ) -> Result<Self> {
        let mut entries = Vec::with_capacity(capacity);
        let mut rng = seed;
        if !loaded.is_empty() {
            if loaded.len() > capacity {
                return Err("loaded positions exceed fixed table budget".into());
            }
            entries.extend_from_slice(loaded);
        } else {
            let wanted = work
                .positions
                .unwrap_or(capacity as u64)
                .min(capacity as u64) as usize;
            let mut cursor = 0u64;
            for i in 0..wanted {
                let len = if work.power_sizes {
                    let choices = (work.max_block / work.block).ilog2() as u64 + 1;
                    work.block << (pattern::random(&mut rng) % choices)
                } else {
                    work.block
                        * (1 + (pattern::random(&mut rng) % ((work.max_block / work.block) as u64))
                            as usize)
                };
                let space = hi - lo - len as u64;
                let blocks = space / work.block as u64 + 1;
                let offset = match work.mode {
                    PositionMode::Sample => pattern::random(&mut rng) % blocks * work.block as u64,
                    PositionMode::Spaced | PositionMode::Alternating => {
                        let pos = if work.mode == PositionMode::Alternating {
                            if i % 2 == 0 {
                                i / 2
                            } else {
                                wanted - 1 - i / 2
                            }
                        } else {
                            i
                        };
                        if wanted <= 1 {
                            0
                        } else {
                            ((pos as u128 * (blocks - 1) as u128) / (wanted - 1) as u128) as u64
                                * work.block as u64
                        }
                    }
                    PositionMode::First => {
                        if cursor + len as u64 > hi - lo {
                            break;
                        }
                        let pos = cursor;
                        cursor += len as u64;
                        pos
                    }
                };
                let write = if work.read_probability == 0. {
                    true
                } else if work.read_probability == 1. {
                    false
                } else {
                    pattern::random(&mut rng) as f64 / u64::MAX as f64 >= work.read_probability
                };
                entries.push(Request {
                    offset: lo + offset,
                    len,
                    write,
                    ..Request::default()
                });
            }
            if !work.from_zero {
                let shift = pattern::random(&mut rng) % ((hi - lo) / work.block as u64);
                for r in &mut entries {
                    let slots = (hi - lo - r.len as u64) / work.block as u64 + 1;
                    r.offset = lo
                        + ((r.offset - lo) / work.block as u64 + shift) % slots * work.block as u64;
                }
            }
            let probability = work.sequential.abs().min(1.);
            if probability < 1. {
                for i in (1..entries.len()).rev() {
                    if pattern::random(&mut rng) as f64 / u64::MAX as f64 >= probability {
                        let j = (pattern::random(&mut rng) % (i as u64 + 1)) as usize;
                        entries.swap(i, j);
                    }
                }
            }
        }
        if entries.is_empty() {
            return Err("no positions fit the worker range".into());
        }
        for (i, request) in entries.iter_mut().enumerate() {
            request.verify = false;
            request.state = 0;
            if work.unique {
                request.seed = (seed as u16).wrapping_add(i as u16);
            }
        }
        Ok(Self {
            entries,
            rng,
            pass: 0,
            lo,
            hi,
            work: work.clone(),
            pending_checks: 0,
        })
    }
    pub fn len(&self) -> u64 {
        self.entries.len() as u64 * if self.work.verify { 2 } else { 1 }
    }
    pub fn phase_remaining(&self, index: u64) -> u64 {
        if self.work.unique_verify {
            return 1;
        }
        let span = self.entries.len() as u64;
        span - index % span
    }
    fn mapped(&self, index: u64) -> (usize, bool) {
        let local = (index % self.len()) as usize;
        let (mut pos, check) = if self.work.unique_verify {
            (local / 2, local % 2 == 1)
        } else {
            (
                local % self.entries.len(),
                self.work.verify && local >= self.entries.len(),
            )
        };
        let stripes = (self.work.sequential.abs() as usize)
            .max(1)
            .min(self.entries.len());
        if stripes > 1 {
            let rows = self.entries.len() / stripes;
            let extra = self.entries.len() % stripes;
            pos = if pos < rows * stripes {
                let stripe = pos % stripes;
                stripe * rows + stripe.min(extra) + pos / stripes
            } else {
                (pos - rows * stripes + 1) * (rows + 1) - 1
            };
        }
        if self.work.sequential < 0. {
            pos = self.entries.len() - 1 - pos;
        }
        (pos, check)
    }
    fn shifted(&self, mut r: Request, pass: u64) -> Request {
        if self.work.shift {
            let slots = (self.hi - self.lo - r.len as u64) / self.work.block as u64 + 1;
            r.offset = self.lo
                + (((r.offset - self.lo) / self.work.block as u64) + pass % slots) % slots
                    * self.work.block as u64;
        }
        r
    }
    pub fn get(&mut self, index: u64) -> Request {
        let pass = index / self.len();
        if pass != self.pass {
            self.pass = pass;
            if self.work.shuffle {
                debug_assert_eq!(self.pending_checks, 0);
                for i in (1..self.entries.len()).rev() {
                    let j = (pattern::random(&mut self.rng) % (i as u64 + 1)) as usize;
                    self.entries.swap(i, j);
                }
            }
        }
        let (pos, check) = self.mapped(index);
        let mut r = self.entries[pos];
        r.verify = check && r.write;
        if check {
            r.write = false;
        }
        if self.work.alternate && pass % 2 == 1 {
            r.write = !r.write;
        }
        self.shifted(r, pass)
    }
    pub fn written(&mut self, index: u64) {
        let (pos, _) = self.mapped(index);
        if !self.entries[pos].verify {
            self.entries[pos].verify = true;
            self.pending_checks += 1;
        }
    }
    pub fn checked(&mut self, index: u64) {
        let (pos, _) = self.mapped(index);
        if self.entries[pos].verify {
            self.entries[pos].verify = false;
            self.pending_checks -= 1;
        }
    }
    pub fn take_pending_check(&mut self, cursor: &mut usize) -> Option<Request> {
        while self.pending_checks > 0 && *cursor < self.entries.len() {
            let pos = *cursor;
            *cursor += 1;
            if self.entries[pos].verify {
                let mut request = self.entries[pos];
                self.entries[pos].verify = false;
                self.pending_checks -= 1;
                request.write = false;
                request.verify = true;
                return Some(self.shifted(request, self.pass));
            }
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stripe_indices_are_permutations() {
        for count in 1..50 {
            for stripes in 1..50 {
                let w = crate::cli::workload(&format!("rs{stripes}k4z")).unwrap();
                let mut p = Positions::new(&w, 0, count * 4096, 1, count as usize, &[]).unwrap();
                let mut offsets = (0..count).map(|i| p.get(i).offset).collect::<Vec<_>>();
                offsets.sort_unstable();
                assert_eq!(offsets, (0..count).map(|i| i * 4096).collect::<Vec<_>>());
            }
        }
    }
    #[test]
    fn linear_variable_sizes_stay_in_range() {
        for syntax in ["ws1k4-16z", "ws0k4:64z", "rs-2k4-16P-50"] {
            let w = crate::cli::workload(syntax).unwrap();
            let mut p = Positions::new(&w, 4096, 1048576, 17, 100, &[]).unwrap();
            for i in 0..1000 {
                let r = p.get(i);
                assert!(r.offset >= 4096 && r.offset + r.len as u64 <= 1048576);
                assert!((w.block..=w.max_block).contains(&r.len));
            }
        }
    }
}
