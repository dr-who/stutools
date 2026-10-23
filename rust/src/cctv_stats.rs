use std::io::{self, Write};

#[derive(Clone, Copy, Default, Debug)]
pub struct WriteTag {
    pub camera: usize,
    pub sequence: u64,
    pub offset: u64,
    pub submitted_ns: u64,
    pub completed_ns: u64,
}
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
    pub max_write_tag: WriteTag,
    pub writes_over_200ms: u64,
    pub gen_ns: u128,
    pub max_gen_ns: u64,
    pub closes: u64,
    pub close_ns: u128,
    pub max_close_ns: u64,
    pub max_close_queue: u64,
}
impl Metrics {
    pub fn merge(&mut self, other: Self) {
        if other.writes > 0 && (self.writes == 0 || other.max_write_ns > self.max_write_ns) {
            self.max_write_tag = other.max_write_tag;
        }
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
        self.writes_over_200ms += other.writes_over_200ms;
        self.gen_ns += other.gen_ns;
        self.max_gen_ns = self.max_gen_ns.max(other.max_gen_ns);
        self.closes += other.closes;
        self.close_ns += other.close_ns;
        self.max_close_ns = self.max_close_ns.max(other.max_close_ns);
        self.max_close_queue = self.max_close_queue.max(other.max_close_queue);
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
pub const HEADER: &str = "period,utc_start,elapsed_start_s,seconds,status,write_ops,write_bytes,avg_write_ms,max_write_ms,preallocated_files,avg_preallocation_ms,max_preallocation_ms,deleted_files,file_rotations,preallocation_waits,avg_preallocation_wait_ms,max_preallocation_wait_ms,max_write_camera,max_write_sequence,max_write_offset,max_write_submit_s,max_write_complete_s,writes_over_200ms,avg_gen_ms,max_gen_ms,closes,avg_close_ms,max_close_ms,max_close_queue\n";

fn padded_csv(out: &mut impl Write, row: &str) -> io::Result<()> {
    for (index, (name, value)) in HEADER
        .trim_end()
        .split(',')
        .zip(row.trim_end().split(','))
        .enumerate()
    {
        if index > 0 {
            write!(out, ", ")?;
        }
        let width = name.len().max(match index {
            1 | 6 => 20,
            _ => 12,
        });
        write!(out, "{value:>width$}")?;
    }
    writeln!(out)
}
pub fn write_header(out: &mut impl Write) -> io::Result<()> {
    padded_csv(out, HEADER)
}

fn row(
    out: &mut impl Write,
    index: u64,
    start: u64,
    seconds: f64,
    utc: u64,
    complete: bool,
    m: Metrics,
) -> io::Result<()> {
    let line = format!(
        "{},{},{start},{seconds:.6},{},{},{},{:.6},{:.6},{},{:.6},{:.6},{},{},{},{:.6},{:.6},{},{},{},{:.9},{:.9},{},{:.6},{:.6},{},{:.6},{:.6},{}",
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
        m.max_preallocation_wait_ns as f64 / 1e6,
        m.max_write_tag.camera,
        m.max_write_tag.sequence,
        m.max_write_tag.offset,
        m.max_write_tag.submitted_ns as f64 / 1e9,
        m.max_write_tag.completed_ns as f64 / 1e9,
        m.writes_over_200ms,
        avg_ms(m.gen_ns, m.writes),
        m.max_gen_ns as f64 / 1e6,
        m.closes,
        avg_ms(m.close_ns, m.closes),
        m.max_close_ns as f64 / 1e6,
        m.max_close_queue
    );
    padded_csv(out, &line)
}
fn avg_ms(total: u128, count: u64) -> f64 {
    if count == 0 {
        0.
    } else {
        total as f64 / count as f64 / 1e6
    }
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
    fn slowest_buffer_keeps_its_camera_and_submit_complete_times() {
        let mut metrics = Metrics {
            writes: 1,
            bytes: 65536,
            write_ns: 45_000_000,
            max_write_ns: 45_000_000,
            max_write_tag: WriteTag {
                camera: 7,
                sequence: 9,
                offset: 65536,
                submitted_ns: 1_000_000_000,
                completed_ns: 1_045_000_000,
            },
            ..Metrics::default()
        };
        let mut out = Vec::new();
        row(&mut out, 0, 0, 60., 0, true, metrics).unwrap();
        let text = String::from_utf8(out).unwrap();
        let fields: Vec<_> = text.trim_end().split(',').map(str::trim).collect();
        assert_eq!(&fields[7..9], &["45.000000", "45.000000"]);
        assert_eq!(
            &fields[17..22],
            &["7", "9", "65536", "1.000000000", "1.045000000"]
        );
        let slow = Metrics {
            writes: 1,
            write_ns: 250_000_000,
            max_write_ns: 250_000_000,
            writes_over_200ms: 1,
            max_write_tag: WriteTag {
                camera: 3,
                submitted_ns: 2_000_000_000,
                completed_ns: 2_250_000_000,
                ..WriteTag::default()
            },
            ..Metrics::default()
        };
        metrics.merge(slow);
        metrics.merge(Metrics {
            writes: 1,
            write_ns: 1_000_000,
            max_write_ns: 1_000_000,
            ..Metrics::default()
        });
        assert_eq!(metrics.max_write_tag.camera, 3);
        assert_eq!(
            metrics.max_write_tag.completed_ns - metrics.max_write_tag.submitted_ns,
            metrics.max_write_ns
        );
        assert_eq!(metrics.writes_over_200ms, 1);
    }
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
            let fields: Vec<_> = line.split(',').map(str::trim).collect();
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
        let fields: Vec<_> = row.trim_end().split(',').map(str::trim).collect();
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
        assert_eq!(&fields[13..17], &["60", "15", "2.000000", "2.000000"]);
    }
}
