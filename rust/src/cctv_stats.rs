use std::io::{self, Write};

#[derive(Clone, Copy, Default, Debug)]
pub struct Metrics {
    pub writes: u64,
    pub bytes: u64,
    pub write_ns: u128,
    pub max_write_ns: u64,
    pub allocations: u64,
    pub allocation_ns: u128,
    pub max_allocation_ns: u64,
    pub deleted: u64,
    pub rotations: u64,
    pub preallocation_waits: u64,
    pub preallocation_wait_ns: u128,
    pub max_preallocation_wait_ns: u64,
}
impl Metrics {
    pub fn merge(&mut self, other: Self) {
        self.writes += other.writes;
        self.bytes += other.bytes;
        self.write_ns += other.write_ns;
        self.max_write_ns = self.max_write_ns.max(other.max_write_ns);
        self.allocations += other.allocations;
        self.allocation_ns += other.allocation_ns;
        self.max_allocation_ns = self.max_allocation_ns.max(other.max_allocation_ns);
        self.deleted += other.deleted;
        self.rotations += other.rotations;
        self.preallocation_waits += other.preallocation_waits;
        self.preallocation_wait_ns += other.preallocation_wait_ns;
        self.max_preallocation_wait_ns = self
            .max_preallocation_wait_ns
            .max(other.max_preallocation_wait_ns);
    }
}
#[derive(Default)]
pub struct MetricsStore {
    slots: [Option<(u64, Metrics)>; 2],
    pub total: Metrics,
}
impl MetricsStore {
    pub fn drain(&mut self, seconds: f64, finish: bool) -> Self {
        let minute = (seconds / 60.) as u64;
        let mut drained = Self::default();
        for (source, target) in self.slots.iter_mut().zip(&mut drained.slots) {
            if source.is_some_and(|(id, _)| id < minute || (finish && id == minute)) {
                *target = source.take();
            }
        }
        drained
    }
    pub fn record(&mut self, seconds: f64, value: Metrics) -> io::Result<()> {
        let minute = (seconds / 60.) as u64;
        let slot = &mut self.slots[(minute % 2) as usize];
        if slot.is_some_and(|(id, _)| id != minute) {
            return Err(io::Error::other(
                "CCTV log writer fell over a minute behind; refusing to lose latency samples",
            ));
        }
        let (_, stats) = slot.get_or_insert((minute, Metrics::default()));
        stats.merge(value);
        self.total.merge(value);
        Ok(())
    }
    fn take(&mut self, minute: u64) -> Metrics {
        let slot = &mut self.slots[(minute % 2) as usize];
        if slot.is_some_and(|(id, _)| id == minute) {
            slot.take().unwrap().1
        } else {
            Metrics::default()
        }
    }
}
pub const HEADER: &str = "period,utc_start,elapsed_start_s,seconds,status,write_ops,write_bytes,avg_write_ms,max_write_ms,preallocated_files,avg_preallocation_ms,max_preallocation_ms,deleted_files,file_rotations,preallocation_waits,avg_preallocation_wait_ms,max_preallocation_wait_ms\n";

fn row(
    out: &mut impl Write,
    index: u64,
    start: u64,
    seconds: f64,
    utc: u64,
    complete: bool,
    m: Metrics,
) -> io::Result<()> {
    writeln!(
        out,
        "{},{},{start},{seconds:.6},{},{},{},{:.6},{:.6},{},{:.6},{:.6},{},{},{},{:.6},{:.6}",
        index + 1,
        super::files::utc(utc + start, true),
        if complete { "complete" } else { "partial" },
        m.writes,
        m.bytes,
        if m.writes == 0 {
            0.
        } else {
            m.write_ns as f64 / m.writes as f64 / 1e6
        },
        m.max_write_ns as f64 / 1e6,
        m.allocations,
        if m.allocations == 0 {
            0.
        } else {
            m.allocation_ns as f64 / m.allocations as f64 / 1e6
        },
        m.max_allocation_ns as f64 / 1e6,
        m.deleted,
        m.rotations,
        m.preallocation_waits,
        if m.preallocation_waits == 0 {
            0.
        } else {
            m.preallocation_wait_ns as f64 / m.preallocation_waits as f64 / 1e6
        },
        m.max_preallocation_wait_ns as f64 / 1e6
    )
}
#[derive(Default)]
pub struct Periods {
    minute: u64,
    hour: u64,
    hour_metrics: Metrics,
}
impl Periods {
    pub fn advance(
        &mut self,
        elapsed: f64,
        finish: bool,
        store: &mut MetricsStore,
        utc: u64,
        minutes: &mut impl Write,
        hours: &mut impl Write,
    ) -> io::Result<bool> {
        let mut wrote_hour = false;
        while elapsed >= (self.minute + 1) as f64 * 60. {
            let m = store.take(self.minute);
            row(minutes, self.minute, self.minute * 60, 60., utc, true, m)?;
            self.hour_metrics.merge(m);
            self.minute += 1;
            if self.minute % 60 == 0 {
                row(
                    hours,
                    self.hour,
                    self.hour * 3600,
                    3600.,
                    utc,
                    true,
                    self.hour_metrics,
                )?;
                self.hour += 1;
                self.hour_metrics = Metrics::default();
                wrote_hour = true;
            }
        }
        if finish {
            let partial = elapsed - self.minute as f64 * 60.;
            if partial > 1e-9 {
                let m = store.take(self.minute);
                row(
                    minutes,
                    self.minute,
                    self.minute * 60,
                    partial,
                    utc,
                    false,
                    m,
                )?;
                self.hour_metrics.merge(m);
            }
            let partial = elapsed - self.hour as f64 * 3600.;
            if partial > 1e-9 {
                row(
                    hours,
                    self.hour,
                    self.hour * 3600,
                    partial,
                    utc,
                    false,
                    self.hour_metrics,
                )?;
                wrote_hour = true;
            }
        }
        minutes.flush()?;
        hours.flush()?;
        Ok(wrote_hour)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seven_days_have_168_flushed_hours_with_weighted_latencies() {
        let mut periods = Periods::default();
        let mut store = MetricsStore::default();
        let mut minutes = Vec::new();
        let mut hours = Vec::new();
        for minute in 0..7 * 24 * 60 {
            store
                .record(
                    minute as f64 * 60. + 1.,
                    Metrics {
                        writes: 1,
                        bytes: 65536,
                        write_ns: 1_000_000,
                        max_write_ns: 1_000_000,
                        ..Metrics::default()
                    },
                )
                .unwrap();
            store
                .record(
                    minute as f64 * 60. + 2.,
                    Metrics {
                        writes: 3,
                        bytes: 3 * 65536,
                        write_ns: 9_000_000,
                        max_write_ns: 3_000_000,
                        ..Metrics::default()
                    },
                )
                .unwrap();
            let flushed = periods
                .advance(
                    (minute + 1) as f64 * 60.,
                    false,
                    &mut store,
                    0,
                    &mut minutes,
                    &mut hours,
                )
                .unwrap();
            assert_eq!(flushed, (minute + 1) % 60 == 0);
            assert_eq!(
                hours.iter().filter(|b| **b == b'\n').count(),
                (minute + 1) as usize / 60
            );
        }
        periods
            .advance(7. * 86400., true, &mut store, 0, &mut minutes, &mut hours)
            .unwrap();
        let hours = String::from_utf8(hours).unwrap();
        assert_eq!(hours.lines().count(), 168);
        for line in hours.lines() {
            let fields: Vec<_> = line.split(',').collect();
            assert_eq!(
                &fields[4..9],
                &["complete", "240", "15728640", "2.500000", "3.000000"]
            );
        }
        assert_eq!(minutes.iter().filter(|b| **b == b'\n').count(), 10080);
    }
    #[test]
    fn minute_and_hour_edges_and_partial_hours_do_not_duplicate() {
        for (elapsed, expected_hours, expected_minutes) in
            [(1., 1, 1), (3600., 1, 60), (3601., 2, 61), (7200., 2, 120)]
        {
            let mut p = Periods::default();
            let mut m = Vec::new();
            let mut h = Vec::new();
            p.advance(
                elapsed,
                true,
                &mut MetricsStore::default(),
                0,
                &mut m,
                &mut h,
            )
            .unwrap();
            assert_eq!(h.iter().filter(|b| **b == b'\n').count(), expected_hours);
            assert_eq!(m.iter().filter(|b| **b == b'\n').count(), expected_minutes);
        }
    }

    #[test]
    fn fifteen_minutes_flush_one_partial_hour_with_all_latency_samples() {
        let mut p = Periods::default();
        let mut store = MetricsStore::default();
        let mut minutes = Vec::new();
        let mut hours = Vec::new();
        for minute in 0..15 {
            store
                .record(
                    minute as f64 * 60. + 1.,
                    Metrics {
                        writes: 2,
                        bytes: 131072,
                        write_ns: 6_000_000,
                        max_write_ns: 4_000_000,
                        rotations: 4,
                        preallocation_waits: 1,
                        preallocation_wait_ns: 2_000_000,
                        max_preallocation_wait_ns: 2_000_000,
                        ..Metrics::default()
                    },
                )
                .unwrap();
            p.advance(
                (minute + 1) as f64 * 60.,
                false,
                &mut store,
                0,
                &mut minutes,
                &mut hours,
            )
            .unwrap();
            assert!(hours.is_empty());
        }
        assert!(p
            .advance(900., true, &mut store, 0, &mut minutes, &mut hours)
            .unwrap());
        assert_eq!(minutes.iter().filter(|b| **b == b'\n').count(), 15);
        let row = String::from_utf8(hours).unwrap();
        let fields: Vec<_> = row.trim_end().split(',').collect();
        assert_eq!(
            &fields[2..9],
            &[
                "0",
                "900.000000",
                "partial",
                "30",
                "1966080",
                "3.000000",
                "4.000000"
            ]
        );
        assert_eq!(&fields[13..], &["60", "15", "2.000000", "2.000000"]);
    }
}
