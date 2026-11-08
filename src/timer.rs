//! Pure timer logic. No UI code in here, so it can be unit-tested.
//!
//! Every method that needs "the current time" takes `now: Instant` as a parameter
//! instead of calling `Instant::now()` itself. That lets the tests pretend time
//! has passed (`start + Duration::from_secs(60)`) without actually waiting.

use std::time::{Duration, Instant};

use crate::schedule::{Schedule, Step};

// `#[derive(...)]` asks the compiler to write some standard code for us:
// - Clone/Copy: a `Mode` can be copied freely like a number (no ownership worries).
// - PartialEq/Eq: lets us compare with `==`.
// - Debug: lets us print it with `{:?}` (and makes test failures readable).
/// Decides an interval's color and chime. Which one a step gets comes from its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Sit,
    Stand,
    /// Any label other than sit/stand: break, walk, lunch...
    Break,
}

/// Something the timer tells the outside world about. The UI decides how to react
/// (sound, flash, notification); the timer doesn't know any of that exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEvent {
    /// The timer has moved on to the next step; see `Timer::step`.
    IntervalEnded,
}

pub struct Timer {
    schedule: Schedule,
    /// Which step of the schedule we're on.
    index: usize,
    /// Time already counted in earlier running stretches (before the last pause).
    elapsed_before_pause: Duration,
    /// `Some(instant)` while running: when the current running stretch began.
    /// `None` while paused. `Option` is Rust's "maybe a value" type; there is no null.
    running_since: Option<Instant>,
    /// When an interval ends: `true` = start the next one right away,
    /// `false` = move to the next step but stay paused until the user starts it.
    /// This is the app setting; the schedule and its steps can override it (see `tick`).
    auto_continue: bool,
}

impl Timer {
    pub fn new(schedule: Schedule) -> Self {
        Timer {
            schedule,
            index: 0,
            elapsed_before_pause: Duration::ZERO,
            running_since: None,
            auto_continue: true,
        }
    }

    pub fn set_auto_continue(&mut self, auto_continue: bool) {
        self.auto_continue = auto_continue;
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    /// The interval being timed now.
    pub fn step(&self) -> &Step {
        self.upcoming(0)
    }

    /// The interval after this one (the schedule wraps around at the end).
    pub fn next_step(&self) -> &Step {
        self.upcoming(1)
    }

    /// The interval `n` steps from now: 0 = this one, 1 = next, and so on.
    pub fn upcoming(&self, n: usize) -> &Step {
        self.schedule.step(self.index + n)
    }

    pub fn is_running(&self) -> bool {
        self.running_since.is_some()
    }

    pub fn current_duration(&self) -> Duration {
        self.step().duration
    }

    /// Swap in a new schedule, staying on the same step number (and keeping the
    /// time counted so far) if the new schedule has one. Used when a duration is
    /// edited; to begin a different schedule from the top, follow with `restart_schedule`.
    pub fn set_schedule(&mut self, schedule: Schedule) {
        if self.index >= schedule.step_count() {
            self.index = 0;
        }
        self.schedule = schedule;
    }

    /// Total time counted in the current interval.
    pub fn elapsed(&self, now: Instant) -> Duration {
        // `if let` runs the block only if the Option holds a value, and binds it to `since`.
        if let Some(since) = self.running_since {
            self.elapsed_before_pause + now.saturating_duration_since(since)
        } else {
            self.elapsed_before_pause
        }
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        // `saturating_sub` stops at zero instead of panicking on underflow.
        self.current_duration().saturating_sub(self.elapsed(now))
    }

    /// 0.0 at the start of an interval, 1.0 at the end.
    pub fn progress(&self, now: Instant) -> f32 {
        let total = self.current_duration().as_secs_f32();
        if total <= 0.0 {
            return 1.0;
        }
        (self.elapsed(now).as_secs_f32() / total).min(1.0)
    }

    pub fn start(&mut self, now: Instant) {
        if self.running_since.is_none() {
            self.running_since = Some(now);
        }
    }

    pub fn pause(&mut self, now: Instant) {
        // `take()` moves the value out of the Option and leaves `None` behind,
        // which is exactly "stop running and remember when we started".
        if let Some(since) = self.running_since.take() {
            self.elapsed_before_pause += now.saturating_duration_since(since);
        }
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.is_running() {
            self.pause(now);
        } else {
            self.start(now);
        }
    }

    /// Start the current interval over from its full length. Keeps running if it was running.
    pub fn restart_interval(&mut self, now: Instant) {
        self.elapsed_before_pause = Duration::ZERO;
        if self.is_running() {
            self.running_since = Some(now);
        }
    }

    /// Go back to the first step of the schedule. Keeps running if it was running.
    pub fn restart_schedule(&mut self, now: Instant) {
        self.index = 0;
        self.restart_interval(now);
    }

    /// Jump to the next step with a fresh interval. Keeps running if it was running.
    pub fn skip(&mut self, now: Instant) {
        let keep_running = self.is_running();
        self.advance(now, keep_running);
    }

    fn advance(&mut self, now: Instant, keep_running: bool) {
        self.index = (self.index + 1) % self.schedule.step_count();
        self.elapsed_before_pause = Duration::ZERO;
        self.running_since = if keep_running { Some(now) } else { None };
    }

    /// Call this regularly (every frame is fine). If the interval has run out,
    /// moves to the next step and returns an event. Whether the new interval
    /// starts running: its own `[auto]`/`[wait]` if it has one, otherwise the
    /// schedule's `start next`, otherwise `auto_continue` (the app setting).
    pub fn tick(&mut self, now: Instant) -> Option<TimerEvent> {
        if self.is_running() && self.remaining(now).is_zero() {
            let auto = self
                .next_step()
                .options
                .auto_start
                .or(self.schedule.settings().auto_start)
                .unwrap_or(self.auto_continue);
            self.advance(now, auto);
            Some(TimerEvent::IntervalEnded)
        } else {
            None
        }
    }
}

// `#[cfg(test)]` means this module is only compiled for `cargo test`.
#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    fn timer() -> Timer {
        Timer::new(Schedule::simple(30 * MIN, 10 * MIN))
    }

    #[test]
    fn starts_paused_in_sit_mode() {
        let t = timer();
        let now = Instant::now();
        assert_eq!(t.step().mode, Mode::Sit);
        assert!(!t.is_running());
        assert_eq!(t.remaining(now), 30 * MIN);
    }

    #[test]
    fn counts_down_while_running() {
        let mut t = timer();
        let t0 = Instant::now();
        t.start(t0);
        assert_eq!(t.remaining(t0 + 5 * MIN), 25 * MIN);
    }

    #[test]
    fn pause_freezes_the_countdown() {
        let mut t = timer();
        let t0 = Instant::now();
        t.start(t0);
        t.pause(t0 + 5 * MIN);
        // An hour later, still paused: nothing more should have been counted.
        assert_eq!(t.remaining(t0 + 65 * MIN), 25 * MIN);
        t.start(t0 + 65 * MIN);
        assert_eq!(t.remaining(t0 + 70 * MIN), 20 * MIN);
    }

    #[test]
    fn tick_switches_mode_and_emits_event_when_time_is_up() {
        let mut t = timer();
        let t0 = Instant::now();
        t.start(t0);
        assert_eq!(t.tick(t0 + 29 * MIN), None);

        let end = t0 + 30 * MIN;
        assert_eq!(t.tick(end), Some(TimerEvent::IntervalEnded));
        assert_eq!(t.step().mode, Mode::Stand);
        assert!(t.is_running());
        assert_eq!(t.remaining(end), 10 * MIN);
    }

    #[test]
    fn without_auto_continue_next_interval_waits_paused() {
        let mut t = timer();
        t.set_auto_continue(false);
        let t0 = Instant::now();
        t.start(t0);

        let end = t0 + 30 * MIN;
        assert_eq!(t.tick(end), Some(TimerEvent::IntervalEnded));
        assert_eq!(t.step().mode, Mode::Stand);
        assert!(!t.is_running());
        // Nobody clicked for 20 minutes: the stand interval must not have started.
        assert_eq!(t.remaining(end + 20 * MIN), 10 * MIN);
    }

    #[test]
    fn restart_interval_resets_time_but_keeps_mode() {
        let mut t = timer();
        let t0 = Instant::now();
        t.start(t0);
        t.restart_interval(t0 + 12 * MIN);
        assert_eq!(t.step().mode, Mode::Sit);
        assert!(t.is_running());
        assert_eq!(t.remaining(t0 + 12 * MIN), 30 * MIN);
    }

    #[test]
    fn tick_does_nothing_while_paused() {
        let mut t = timer();
        assert_eq!(t.tick(Instant::now() + 999 * MIN), None);
    }

    #[test]
    fn skip_switches_mode_and_keeps_paused_state() {
        let mut t = timer();
        let t0 = Instant::now();
        t.skip(t0);
        assert_eq!(t.step().mode, Mode::Stand);
        assert!(!t.is_running());
        assert_eq!(t.remaining(t0), 10 * MIN);
    }

    #[test]
    fn progress_goes_from_zero_to_one() {
        let mut t = timer();
        let t0 = Instant::now();
        t.start(t0);
        assert_eq!(t.progress(t0), 0.0);
        assert_eq!(t.progress(t0 + 15 * MIN), 0.5);
    }

    #[test]
    fn walks_through_a_custom_schedule_and_wraps_around() {
        let schedule = Schedule::parse("2x\n- 1m sit\n- 2m stand\n\n- 3m break").unwrap();
        let mut t = Timer::new(schedule);
        let mut now = Instant::now();
        t.start(now);
        let mut seen = vec![t.step().label.clone()];
        for _ in 0..5 {
            now += t.remaining(now);
            assert_eq!(t.tick(now), Some(TimerEvent::IntervalEnded));
            seen.push(t.step().label.clone());
        }
        assert_eq!(seen, ["sit", "stand", "sit", "stand", "break", "sit"]);
        assert_eq!(t.next_step().label, "stand");
    }

    #[test]
    fn set_schedule_keeps_position_when_it_can() {
        let mut t = timer();
        let t0 = Instant::now();
        t.skip(t0); // now on stand
        t.set_schedule(Schedule::simple(30 * MIN, 20 * MIN));
        assert_eq!(t.step().mode, Mode::Stand);
        assert_eq!(t.remaining(t0), 20 * MIN);

        t.set_schedule(Schedule::parse("- 5m sit").unwrap());
        assert_eq!(t.remaining(t0), 5 * MIN);
    }

    #[test]
    fn step_wait_and_auto_beat_the_schedule_and_app_settings() {
        // The schedule says wait; "stand" overrides it with [auto].
        let text = "*****\nstart next: wait\n*****\n- 1m sit\n- 1m stand [auto]\n- 1m walk";
        let mut t = Timer::new(Schedule::parse(text).unwrap());
        t.set_auto_continue(true); // the app setting, overridden by the schedule
        let t0 = Instant::now();
        t.start(t0);
        t.tick(t0 + MIN);
        assert_eq!(t.step().label, "stand");
        assert!(t.is_running(), "[auto] on stand");
        t.tick(t0 + 2 * MIN);
        assert_eq!(t.step().label, "walk");
        assert!(!t.is_running(), "start next: wait");
    }

    #[test]
    fn restart_schedule_goes_back_to_the_first_step() {
        let mut t = timer();
        let t0 = Instant::now();
        t.start(t0);
        t.skip(t0 + MIN);
        t.restart_schedule(t0 + 2 * MIN);
        assert_eq!(t.step().mode, Mode::Sit);
        assert!(t.is_running());
        assert_eq!(t.remaining(t0 + 2 * MIN), 30 * MIN);
    }
}
