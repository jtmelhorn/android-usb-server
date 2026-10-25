// SPDX-License-Identifier: GPL-3.0-or-later
//! Counters and timing samples. Owned by the server thread (no locking on the input path);
//! a JSON snapshot is published periodically for the UI.
//!
//! What each timing measures — keep these definitions in sync with docs and UI labels:
//! * `report_interval_us`: time between consecutive successful interrupt-IN completions that
//!   carried data. This is the device/host *report interval*, NOT input latency.
//! * `turnaround_us`: URB completion reaped on the Shield → RET_SUBMIT fully handed to the TCP
//!   socket. Server-internal processing time only.

use std::fmt::Write as _;
use std::time::Instant;

pub const RING: usize = 1024;

/// Fixed-size ring of microsecond samples; recording never allocates.
pub struct Samples {
    buf: Box<[u32; RING]>,
    len: usize,
    next: usize,
}

impl Default for Samples {
    fn default() -> Self {
        Self { buf: Box::new([0; RING]), len: 0, next: 0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    pub n: usize,
    pub min: u32,
    pub p50: u32,
    pub p99: u32,
    pub max: u32,
    pub avg: u32,
}

impl Samples {
    #[inline]
    pub fn record(&mut self, us: u32) {
        self.buf[self.next] = us;
        self.next = (self.next + 1) % RING;
        if self.len < RING {
            self.len += 1;
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.next = 0;
    }

    /// Summary of the retained samples (sorting a copy; called only for status snapshots).
    pub fn summary(&self) -> Summary {
        if self.len == 0 {
            return Summary::default();
        }
        let mut v: Vec<u32> = self.buf[..self.len].to_vec();
        v.sort_unstable();
        let pct = |p: usize| v[((v.len() - 1) * p + 50) / 100];
        let sum: u64 = v.iter().map(|&x| x as u64).sum();
        Summary {
            n: v.len(),
            min: v[0],
            p50: pct(50),
            p99: pct(99),
            max: v[v.len() - 1],
            avg: (sum / v.len() as u64) as u32,
        }
    }
}

#[derive(Default)]
pub struct Stats {
    pub sessions: u64,
    pub rejected_peers: u64,
    pub submits: u64,
    pub completions: u64,
    pub unlinks: u64,
    pub errors: u64,
    pub last_error_status: i32,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub interrupt_in_reports: u64,
    pub report_interval: Samples,
    pub turnaround: Samples,
    last_report: Option<Instant>,
}

impl Stats {
    #[inline]
    pub fn on_interrupt_in_report(&mut self, now: Instant) {
        self.interrupt_in_reports += 1;
        if let Some(prev) = self.last_report {
            self.report_interval.record(us_between(prev, now));
        }
        self.last_report = Some(now);
    }

    /// Forget the previous report time (e.g. when a session ends) so the gap is not counted.
    pub fn reset_report_clock(&mut self) {
        self.last_report = None;
    }

    pub fn to_json(&self, state: &str, client: &str, detail: &str) -> String {
        let mut s = String::with_capacity(768);
        let _ = write!(
            s,
            "{{\"state\":\"{}\",\"client\":\"{}\",\"detail\":\"{}\",\"sessions\":{},\"rejectedPeers\":{},\
             \"submits\":{},\"completions\":{},\"unlinks\":{},\"errors\":{},\"lastErrorStatus\":{},\
             \"bytesIn\":{},\"bytesOut\":{},\"interruptInReports\":{},",
            esc(state),
            esc(client),
            esc(detail),
            self.sessions,
            self.rejected_peers,
            self.submits,
            self.completions,
            self.unlinks,
            self.errors,
            self.last_error_status,
            self.bytes_in,
            self.bytes_out,
            self.interrupt_in_reports
        );
        summary_json(&mut s, "reportIntervalUs", self.report_interval.summary());
        s.push(',');
        summary_json(&mut s, "turnaroundUs", self.turnaround.summary());
        s.push('}');
        s
    }
}

fn summary_json(s: &mut String, name: &str, x: Summary) {
    let _ = write!(
        s,
        "\"{name}\":{{\"n\":{},\"min\":{},\"p50\":{},\"p99\":{},\"max\":{},\"avg\":{}}}",
        x.n, x.min, x.p50, x.p99, x.max, x.avg
    );
}

fn esc(v: &str) -> String {
    let mut o = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o
}

#[inline]
pub fn us_between(a: Instant, b: Instant) -> u32 {
    b.saturating_duration_since(a).as_micros().min(u32::MAX as u128) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn summary_percentiles() {
        let mut s = Samples::default();
        assert_eq!(s.summary(), Summary::default());
        for v in 1..=100 {
            s.record(v);
        }
        let x = s.summary();
        assert_eq!((x.n, x.min, x.max), (100, 1, 100));
        assert_eq!(x.p50, 51);
        assert_eq!(x.p99, 99);
        assert_eq!(x.avg, 50);
    }

    #[test]
    fn ring_keeps_latest() {
        let mut s = Samples::default();
        for v in 0..(RING as u32 + 10) {
            s.record(v);
        }
        let x = s.summary();
        assert_eq!(x.n, RING);
        assert_eq!(x.min, 10);
        assert_eq!(x.max, RING as u32 + 9);
    }

    #[test]
    fn report_interval_recorded_between_reports() {
        let mut st = Stats::default();
        let t0 = Instant::now();
        st.on_interrupt_in_report(t0);
        st.on_interrupt_in_report(t0 + Duration::from_micros(4000));
        st.on_interrupt_in_report(t0 + Duration::from_micros(8000));
        let x = st.report_interval.summary();
        assert_eq!((x.n, x.min, x.max), (2, 4000, 4000));
        st.reset_report_clock();
        st.on_interrupt_in_report(t0 + Duration::from_secs(10));
        assert_eq!(st.report_interval.summary().n, 2);
    }

    #[test]
    fn json_escapes() {
        let st = Stats::default();
        let j = st.to_json("err\"or", "a\\b", "line\nbreak");
        assert!(j.starts_with("{\"state\":\"err\\\"or\",\"client\":\"a\\\\b\",\"detail\":\"line\\u000abreak\""));
        assert!(j.ends_with("}}"));
    }
}
