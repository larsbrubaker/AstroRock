//! # 30 Hz beat pacing — port of `HeartBeat.cpp`
//!
//! Converts wall time into "how many 30 Hz logic beats should run this
//! frame". `read_and_clear` returns at most `max_beats_to_skip` (5) —
//! on a slow frame the excess simulation time is dropped, exactly like
//! the original (Clear latches the *full* beat count, so skipped beats
//! never come back).
//!
//! The clock is injected as milliseconds so tests are exact and shells
//! can drive it from `web_time::Instant`.

pub const BEATS_PER_SECOND: u64 = 30;
pub const MAX_BEATS_TO_SKIP: u32 = 5;

pub struct HeartBeat {
    start_ms: u64,
    beats_since_start: u64,
    beats_per_second: u64,
    max_beats_to_skip: u32,
}

impl HeartBeat {
    /// `CHeartBeat::CHeartBeat` — anchor the beat counter at `now_ms`.
    pub fn new(now_ms: u64) -> Self {
        Self {
            start_ms: now_ms,
            beats_since_start: 0,
            beats_per_second: BEATS_PER_SECOND,
            max_beats_to_skip: MAX_BEATS_TO_SKIP,
        }
    }

    /// Total beats elapsed since construction at `now_ms`.
    fn total_beats(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.start_ms) * self.beats_per_second / 1000
    }

    /// `CHeartBeat::Read` — beats owed since the last clear, capped.
    pub fn read(&self, now_ms: u64) -> u32 {
        let owed = self.total_beats(now_ms) - self.beats_since_start;
        (owed as u32).min(self.max_beats_to_skip)
    }

    /// `CHeartBeat::ReadAndClear` — beats owed, then latch. Beats beyond
    /// the cap are dropped, not deferred.
    pub fn read_and_clear(&mut self, now_ms: u64) -> u32 {
        let owed = self.read(now_ms);
        self.beats_since_start = self.total_beats(now_ms);
        owed
    }

    pub fn set_max_beats_to_skip(&mut self, max: u32) {
        self.max_beats_to_skip = max;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_beats_per_second() {
        let mut hb = HeartBeat::new(1000);
        assert_eq!(hb.read_and_clear(1000), 0);
        // 33ms → 0 beats (33*30/1000 = 0); 34ms → 1.
        assert_eq!(hb.read_and_clear(1034), 1);
        assert_eq!(hb.read_and_clear(1067), 1); // 67ms total → beat 2
        assert_eq!(hb.read_and_clear(1067), 0);
    }

    #[test]
    fn slow_frame_caps_and_drops() {
        let mut hb = HeartBeat::new(0);
        // A 1-second stall owes 30 beats; only 5 run and the rest drop.
        assert_eq!(hb.read_and_clear(1000), 5);
        assert_eq!(hb.read_and_clear(1000), 0);
        // Time keeps flowing normally afterwards.
        assert_eq!(hb.read_and_clear(1100), 3);
    }

    #[test]
    fn read_without_clear_is_idempotent() {
        let mut hb = HeartBeat::new(0);
        assert_eq!(hb.read(500), 5);
        assert_eq!(hb.read(500), 5);
        assert_eq!(hb.read_and_clear(500), 5);
        assert_eq!(hb.read(500), 0);
    }
}
