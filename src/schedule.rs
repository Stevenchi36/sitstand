//! Schedules: the list of intervals the timer steps through, and the `.sstc`
//! text format for writing your own. Pure logic, no UI, so it can be unit-tested.
//!
//! ```text
//! # Lines starting with `-` are intervals: a length and a label.
//! # `3x` repeats the lines below it. A blank line ends a repeat.
//! 3x
//! - 30m sit
//! - 15m stand
//!
//! - 15m break
//! ```
//!
//! Repeats can be nested by indenting their contents:
//!
//! ```text
//! 4x
//!   2x
//!     - 25m sit
//!     - 5m stand
//!   - 10m break
//! ```
//!
//! The rules:
//! - `- <length> <label>`: an interval. Lengths: `90s`, `30m`, `1h`, `1h30m`.
//!   A label starting with "sit" or "stand" uses that mode's color and chime;
//!   anything else (break, walk, lunch...) uses the break color.
//! - `<N>x`: repeat. If the next line is indented more, the repeat holds the
//!   indented lines. Otherwise it holds the lines that follow, up to a blank line.
//! - `#` starts a comment, to the end of the line, when it begins the line or is
//!   followed by a space. So `# note` is a comment, but `#c0ff33` and `break #2` aren't.
//! - `[...]` at the end of an interval line holds options for the moment that
//!   interval starts, comma-separated: `silent`, `no sound`, `no flash`,
//!   `no notification` (turn parts of its alert off), `wait` / `auto` (wait for a
//!   click before it starts, or start by itself). On an `Nx` line they apply to
//!   everything inside; the "no ..." options add up, and the innermost wait/auto wins.
//! - When the whole schedule is done, it starts over from the top.
//!
//! An optional settings block at the very top, between lines of `*****`, holds
//! `name: value` lines. A value starting with `#` gives that label its own color
//! (matched ignoring case; unlisted labels keep the sit/stand/break colors from
//! the settings panel). Otherwise it sets one of these, in place of the app's
//! setting, while this schedule is loaded (see `apply_setting`):
//!
//! ```text
//! *****
//! alerts: sound only      # all, visual only, sound only or silent
//! start next: wait        # auto or wait (for a click)
//! repeat chime: 60s, 3x   # every 60 s, at most 3 times; or off
//! flash: 15s             # how long the widget flashes, 1s to 10m
//! long break: #c0ff33
//! *****
//! ```

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::Duration;

use crate::config::AlertStyle;
use crate::timer::Mode;

/// Repeats multiply, so a short file could ask for billions of intervals. Refuse past this.
const MAX_STEPS: usize = 10_000;
const MAX_REPEAT: u32 = 1000;
const MAX_STEP_LENGTH: Duration = Duration::from_secs(24 * 60 * 60);

/// Written as `Pomodoro.sstc` when the default schedules folder is first created,
/// so there's a schedule to pick and an example to copy.
pub const POMODORO: &str = "\
# Sit-Stand Timer schedule.
#   - <length> <label>   an interval, e.g. `- 30m sit`, `- 90s stand`, `- 1h30m lunch`
#   <N>x                 repeat the lines below it, up to the next blank line
#                        (or the indented lines below it, which lets repeats nest)
# Labels starting with \"sit\" or \"stand\" get that color; anything else is a break.
# After the last line, the schedule starts over. Edit, save, then press Reload.
#
# Options in [brackets] at the end of a line apply when that interval starts
# (on an `Nx` line, to everything it repeats): silent, no sound, no flash,
# no notification, wait (for a click) or auto. E.g. `- 2m move [silent]`.
#
# Optional, at the very top: settings for this schedule, between lines of *****.
# Each one replaces the app setting while this schedule is loaded.
#   *****
#   alerts: sound only      all, visual only, sound only or silent
#   start next: wait        auto, or wait for a click
#   repeat chime: 60s, 3x   every 60s, at most 3 times; or off
#   flash: 15s              how long the widget flashes
#   walk: #ff8800           a label's color
#   *****

# The pomodoro technique: 4 focus sessions of 25 minutes with a 5-minute break
# after each, except the 4th, which gets a longer break instead.
#
# As in the original technique, you start each focus session yourself ([wait]:
# click the widget, like winding the kitchen timer), while breaks start on
# their own. The long break gets its own color, so it stands out.

*****
start next: auto
long break: #b07cff
*****

3x
- 25m sit [wait]       # focus
- 5m stand             # short break

- 25m sit [wait]       # 4th focus
- 20m long break
";

/// Options from `[...]` at the end of an interval line, or of the `Nx` line of a
/// repeat it's inside. They apply to the moment this interval *starts*.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StepOptions {
    pub no_sound: bool,
    pub no_flash: bool,
    pub no_notification: bool,
    /// `[auto]` = `Some(true)`, `[wait]` = `Some(false)`: whether this interval starts
    /// by itself when the one before it ends. `None`: the schedule's or app's setting.
    pub auto_start: Option<bool>,
}

impl StepOptions {
    /// These options, inside a repeat that has `outer`: the "no ..." options add up,
    /// and the innermost `[auto]`/`[wait]` wins.
    fn within(self, outer: StepOptions) -> StepOptions {
        StepOptions {
            no_sound: self.no_sound || outer.no_sound,
            no_flash: self.no_flash || outer.no_flash,
            no_notification: self.no_notification || outer.no_notification,
            auto_start: self.auto_start.or(outer.auto_start),
        }
    }
}

/// The repeat chime: every `every_secs` seconds, at most `max_times` times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reminder {
    pub every_secs: u32,
    pub max_times: u32,
}

/// App settings a schedule file can set for itself, in its `*****` block.
/// `None` everywhere means "use the app's setting".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScheduleSettings {
    /// `alerts: sound only`
    pub alerts: Option<AlertStyle>,
    /// `start next: auto` = `Some(true)`, `start next: wait` = `Some(false)`.
    pub auto_start: Option<bool>,
    /// `repeat chime: 60s, 3x` = `Some(Some(..))`; `repeat chime: off` = `Some(None)`.
    pub reminder: Option<Option<Reminder>>,
    /// `flash: 15s`: how long the widget flashes when an interval starts.
    pub flash: Option<Duration>,
}

/// Where a step sits inside one repeat: round `current` of `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Round {
    pub current: u32,
    pub total: u32,
}

/// One countdown in the schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub label: String,
    pub duration: Duration,
    pub mode: Mode,
    /// As [red, green, blue], from the file's `*****` block. `None`: use the
    /// settings-panel color for `mode`.
    pub color: Option<[u8; 3]>,
    pub options: StepOptions,
    /// The repeats this step is inside, outermost first. Empty if it isn't repeated.
    pub rounds: Vec<Round>,
}

impl Step {
    /// e.g. "Round 2/3", or "Round 2/4 · 1/2" when nested. `None` outside any repeat.
    pub fn rounds_text(&self) -> Option<String> {
        if self.rounds.is_empty() {
            return None;
        }
        let parts: Vec<String> = self
            .rounds
            .iter()
            .map(|r| format!("{}/{}", r.current, r.total))
            .collect();
        Some(format!("Round {}", parts.join(" · ")))
    }
}

/// The repeats are expanded ("unrolled") when parsing, so this is just the steps
/// in order. Never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    steps: Vec<Step>,
    settings: ScheduleSettings,
}

impl Schedule {
    /// The plain sit/stand cycle from the settings panel.
    pub fn simple(sit: Duration, stand: Duration) -> Schedule {
        let step = |label: &str, duration, mode| Step {
            label: label.to_owned(),
            duration,
            mode,
            color: None,
            options: StepOptions::default(),
            rounds: Vec::new(),
        };
        Schedule {
            steps: vec![
                step("sit", sit, Mode::Sit),
                step("stand", stand, Mode::Stand),
            ],
            settings: ScheduleSettings::default(),
        }
    }

    /// Read a schedule written in the `.sstc` format (see the top of this file).
    pub fn parse(text: &str) -> Result<Schedule, ParseError> {
        let mut lines = split_lines(text);
        let (colors, settings) = take_settings_block(&mut lines)?;
        let mut parser = Parser { lines, pos: 0 };
        let Some(first) = parser.lines.iter().find(|l| !l.is_blank()) else {
            return Err(ParseError::whole_file("the schedule is empty"));
        };
        let nodes = parser.items(first.indent, false)?;
        // `items` only stops early at a line indented less than the first one.
        if let Some(line) = parser.lines.get(parser.pos) {
            return Err(ParseError::at(line.number, "unexpected indentation"));
        }
        let mut steps = Vec::new();
        unroll(&nodes, &mut Vec::new(), StepOptions::default(), &mut steps)?;
        for step in &mut steps {
            step.color = colors.get(&label_key(&step.label)).copied();
        }
        Ok(Schedule { steps, settings })
    }

    /// What the file's `*****` block sets in place of the app settings.
    pub fn settings(&self) -> &ScheduleSettings {
        &self.settings
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Step number `index`, wrapping around past the end (so `index + 1` is always "next").
    pub fn step(&self, index: usize) -> &Step {
        &self.steps[index % self.steps.len()]
    }

    /// How long one pass through the whole schedule takes.
    pub fn cycle_length(&self) -> Duration {
        self.steps.iter().map(|s| s.duration).sum()
    }
}

/// A problem in a schedule file, with the line it's on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line number, or 0 when the problem isn't on one line.
    pub line: usize,
    pub message: String,
}

impl ParseError {
    fn at(line: usize, message: impl Into<String>) -> ParseError {
        ParseError {
            line,
            message: message.into(),
        }
    }

    fn whole_file(message: impl Into<String>) -> ParseError {
        ParseError::at(0, message)
    }
}

// `Display` is what `{}` and `.to_string()` use.
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

/// "pomodoro.sstc" -> "pomodoro": how a schedule file is named in the settings and tray.
pub fn display_name(file_name: &str) -> String {
    std::path::Path::new(file_name).file_stem().map_or_else(
        || file_name.to_owned(),
        |s| s.to_string_lossy().into_owned(),
    )
}

/// "15 minutes", "1 hour 30 minutes", "1 minute 30 seconds".
pub fn describe_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let parts: Vec<String> = [
        (secs / 3600, "hour"),
        (secs / 60 % 60, "minute"),
        (secs % 60, "second"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, unit)| format!("{n} {unit}{}", if n == 1 { "" } else { "s" }))
    .collect();
    if parts.is_empty() {
        "0 seconds".to_owned()
    } else {
        parts.join(" ")
    }
}

fn mode_for_label(label: &str) -> Mode {
    let label = label.to_lowercase();
    if label.starts_with("sit") {
        Mode::Sit
    } else if label.starts_with("stand") {
        Mode::Stand
    } else {
        Mode::Break
    }
}

/// A parsed line of the file, before repeats are unrolled.
enum Node {
    Step {
        label: String,
        duration: Duration,
        options: StepOptions,
    },
    Repeat {
        times: u32,
        options: StepOptions,
        body: Vec<Node>,
    },
}

/// One line of the file, with its comment removed. `Clone`/`Copy` are cheap:
/// `text` only borrows from the original string.
#[derive(Clone, Copy)]
struct Line<'a> {
    number: usize,
    indent: usize,
    /// Trimmed. Empty for a blank line.
    text: &'a str,
}

impl Line<'_> {
    fn is_blank(&self) -> bool {
        self.text.is_empty()
    }
}

fn split_lines(text: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let content = strip_comment(raw);
        if content.trim().is_empty() && !raw.trim().is_empty() {
            continue; // a comment-only line: ignored entirely, it doesn't end a block
        }
        let indent = content
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum();
        lines.push(Line {
            number: i + 1,
            indent,
            text: content.trim(),
        });
    }
    lines
}

struct Parser<'a> {
    lines: Vec<Line<'a>>,
    /// Index of the next line to read.
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<Line<'a>> {
        self.lines.get(self.pos).copied()
    }

    /// The next non-blank line at or after `pos`.
    fn next_non_blank(&self) -> Option<Line<'a>> {
        self.lines[self.pos..]
            .iter()
            .find(|l| !l.is_blank())
            .copied()
    }

    /// Reads consecutive lines indented exactly `indent`. Stops (without consuming)
    /// at a line indented less, and at a blank line when `stop_at_blank` is set.
    fn items(&mut self, indent: usize, stop_at_blank: bool) -> Result<Vec<Node>, ParseError> {
        let mut nodes = Vec::new();
        while let Some(line) = self.peek() {
            if line.is_blank() {
                // In an indented block, blank lines don't matter, unless the block ends
                // right after them: then leave the blank for an outer block to see.
                let block_continues = self.next_non_blank().is_some_and(|l| l.indent >= indent);
                if stop_at_blank || !block_continues {
                    break;
                }
                self.pos += 1;
                continue;
            }
            if line.indent < indent {
                break;
            }
            if line.indent > indent {
                return Err(ParseError::at(line.number, "unexpected indentation"));
            }
            self.pos += 1;
            nodes.push(self.item(line)?);
        }
        Ok(nodes)
    }

    fn item(&mut self, mut line: Line<'a>) -> Result<Node, ParseError> {
        // `[...]` options come off first; the rest of the line is read as before.
        let (text, options) = split_options(line.text, line.number)?;
        line.text = text;
        if let Some(rest) = line.text.strip_prefix('-') {
            return parse_step(rest, options, line.number);
        }
        if is_fence(line.text) {
            return Err(ParseError::at(
                line.number,
                "the ***** settings block has to be at the top, before any intervals",
            ));
        }
        let Some(times) = parse_repeat(line.text) else {
            return Err(ParseError::at(
                line.number,
                format!(
                    "`{}` isn't an interval like `- 30m sit` or a repeat like `3x`",
                    line.text
                ),
            ));
        };
        if !(1..=MAX_REPEAT).contains(&times) {
            return Err(ParseError::at(
                line.number,
                format!("repeat count must be 1 to {MAX_REPEAT}"),
            ));
        }
        let body = match self.peek() {
            // Indented style: the repeat holds every line indented under it.
            Some(next) if !next.is_blank() && next.indent > line.indent => {
                self.items(next.indent, false)?
            }
            // Flat style: the repeat holds the following lines, up to a blank line.
            _ => self.items(line.indent, true)?,
        };
        if body.is_empty() {
            return Err(ParseError::at(
                line.number,
                format!(
                    "`{}` has nothing to repeat: put intervals on the lines right below it",
                    line.text
                ),
            ));
        }
        Ok(Node::Repeat {
            times,
            options,
            body,
        })
    }
}

/// Splits `[...]` options off the end of a line: "- 2m move [silent]" ->
/// ("- 2m move", silent options). Lines without them get the defaults.
fn split_options(text: &str, line: usize) -> Result<(&str, StepOptions), ParseError> {
    let mut options = StepOptions::default();
    let Some(inner) = text.strip_suffix(']') else {
        return Ok((text, options));
    };
    let Some(open) = inner.rfind('[') else {
        return Err(ParseError::at(line, "`]` without a `[` before it"));
    };
    for word in inner[open + 1..].split(',') {
        match label_key(word).as_str() {
            "silent" => {
                options.no_sound = true;
                options.no_flash = true;
                options.no_notification = true;
            }
            "no sound" => options.no_sound = true,
            "no flash" => options.no_flash = true,
            "no notification" | "no notifications" => options.no_notification = true,
            "auto" | "wait" if options.auto_start.is_some() => {
                return Err(ParseError::at(
                    line,
                    "use either `auto` or `wait`, not both",
                ));
            }
            "auto" => options.auto_start = Some(true),
            "wait" => options.auto_start = Some(false),
            _ => {
                return Err(ParseError::at(
                    line,
                    format!(
                        "`{}` isn't an option: use silent, no sound, no flash, \
                         no notification, wait or auto",
                        word.trim()
                    ),
                ));
            }
        }
    }
    Ok((inner[..open].trim_end(), options))
}

/// The part after the `-`, e.g. " 30m sit".
fn parse_step(rest: &str, options: StepOptions, line: usize) -> Result<Node, ParseError> {
    let rest = rest.trim();
    let (length, label) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let label = label.trim();
    if length.is_empty() || label.is_empty() {
        return Err(ParseError::at(
            line,
            "an interval needs a length and a label, like `- 30m sit`",
        ));
    }
    let Some(duration) = parse_duration(length) else {
        return Err(ParseError::at(
            line,
            format!("`{length}` isn't a length: use e.g. 90s, 30m, 1h or 1h30m"),
        ));
    };
    if duration.is_zero() || duration > MAX_STEP_LENGTH {
        return Err(ParseError::at(
            line,
            "an interval must be between 1 second and 24 hours",
        ));
    }
    Ok(Node::Step {
        label: label.to_owned(),
        duration,
        options,
    })
}

/// "1h30m" -> 90 minutes. Each number needs a unit: h, m or s.
fn parse_duration(text: &str) -> Option<Duration> {
    let mut total: u64 = 0;
    let mut number = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let unit = match c.to_ascii_lowercase() {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        // An empty `number` (like "m" or "1hm") fails to parse, which is what we want.
        let n: u64 = number.parse().ok()?;
        number.clear();
        // `checked_*` returns `None` instead of overflowing on absurd numbers.
        total = total.checked_add(n.checked_mul(unit)?)?;
    }
    if !number.is_empty() || text.is_empty() {
        return None; // a number without a unit, like "30"
    }
    Some(Duration::from_secs(total))
}

/// The line without its comment. `#` starts one at the beginning of the line, or
/// when followed by a space (or nothing), so `#c0ff33` and `break #2` keep their `#`.
fn strip_comment(line: &str) -> &str {
    let first_text = line.len() - line.trim_start().len();
    for (i, c) in line.char_indices() {
        if c != '#' {
            continue;
        }
        // `'#'` is one byte long, so `i + 1` is where the next character starts.
        let next = line[i + 1..].chars().next();
        if i == first_text || next.is_none_or(char::is_whitespace) {
            return &line[..i];
        }
    }
    line
}

/// A line of three or more `*`: the top or bottom of the settings block.
fn is_fence(text: &str) -> bool {
    text.len() >= 3 && text.chars().all(|c| c == '*')
}

/// How labels are compared: ignoring case and extra spaces, so `Long  Break`
/// in the settings block matches `- 15m long break`.
fn label_key(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Label colors from the `*****` block, keyed by `label_key`.
type Colors = HashMap<String, [u8; 3]>;

/// If the file starts with a `*****` settings block, reads it and removes it from
/// `lines`. Each line is `label: #color`, or `setting: value` for the settings
/// listed in `apply_setting`. A value starting with `#` is always a color.
fn take_settings_block(lines: &mut Vec<Line>) -> Result<(Colors, ScheduleSettings), ParseError> {
    let mut colors = Colors::new();
    let mut settings = ScheduleSettings::default();
    let Some(open) = lines.iter().position(|l| !l.is_blank()) else {
        return Ok((colors, settings));
    };
    if !is_fence(lines[open].text) {
        return Ok((colors, settings));
    }
    let Some(len) = lines[open + 1..].iter().position(|l| is_fence(l.text)) else {
        return Err(ParseError::at(
            lines[open].number,
            "this ***** settings block is never closed: end it with another line of *****",
        ));
    };
    let close = open + 1 + len;
    // Every name seen so far, to catch one set twice.
    let mut seen = HashSet::new();
    for line in &lines[open + 1..close] {
        if line.is_blank() {
            continue;
        }
        // `rsplit_once`: split at the *last* colon, so a label may contain one.
        let Some((name, value)) = line.text.rsplit_once(':') else {
            return Err(ParseError::at(
                line.number,
                "expected `name: value`, like `long break: #c0ff33` or `alerts: silent`",
            ));
        };
        let key = label_key(name);
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(ParseError::at(
                line.number,
                "expected `name: value`, like `long break: #c0ff33` or `alerts: silent`",
            ));
        }
        // `insert` returns false if the set already had it.
        if !seen.insert(key.clone()) {
            return Err(ParseError::at(
                line.number,
                format!("`{}` is already set above", name.trim()),
            ));
        }
        if value.starts_with('#') {
            let Some(color) = parse_color(value) else {
                return Err(ParseError::at(
                    line.number,
                    format!("`{value}` isn't a color: use #rrggbb or #rgb, like #c0ff33"),
                ));
            };
            colors.insert(key, color);
        } else {
            apply_setting(&mut settings, &key, value)
                .map_err(|message| ParseError::at(line.number, message))?;
        }
    }
    // Drop everything up to the closing line; the intervals start after it.
    lines.drain(..=close);
    Ok((colors, settings))
}

/// One `setting: value` line of the `*****` block. The error is the message to show.
fn apply_setting(settings: &mut ScheduleSettings, key: &str, value: &str) -> Result<(), String> {
    let value_key = label_key(value);
    match key {
        "alerts" => {
            let style = match value_key.as_str() {
                "all" => AlertStyle::All,
                "visual only" => AlertStyle::VisualOnly,
                "sound only" => AlertStyle::SoundOnly,
                "silent" => AlertStyle::Silent,
                _ => {
                    return Err(format!(
                        "`{value}` isn't an alert style: use all, visual only, sound only or silent"
                    ));
                }
            };
            settings.alerts = Some(style);
        }
        "start next" => {
            settings.auto_start = match value_key.as_str() {
                "auto" => Some(true),
                "wait" => Some(false),
                _ => return Err(format!("`{value}` isn't auto or wait")),
            };
        }
        "repeat chime" => {
            settings.reminder = Some(if value_key == "off" {
                None
            } else {
                Some(parse_reminder(value)?)
            });
        }
        "flash" => {
            let flash = parse_duration(value)
                .filter(|d| (1..=600).contains(&d.as_secs()))
                .ok_or_else(|| format!("`{value}` isn't a flash time: use 1s to 10m, like 15s"))?;
            settings.flash = Some(flash);
        }
        _ => {
            return Err(format!(
                "`{key}` isn't a setting (alerts, start next, repeat chime, flash). \
                 For a label's color, use #rrggbb"
            ));
        }
    }
    Ok(())
}

/// "60s, 3x" -> every 60 seconds, at most 3 times. Same limits as the settings panel.
fn parse_reminder(value: &str) -> Result<Reminder, String> {
    let usage = format!("`{value}` isn't a repeat chime: use off, or e.g. `60s, 3x`");
    let (every, times) = value.split_once(',').ok_or(usage.clone())?;
    let every = parse_duration(every.trim()).ok_or(usage.clone())?;
    let max_times = parse_repeat(times.trim()).ok_or(usage)?;
    let every_secs = u32::try_from(every.as_secs()).unwrap_or(u32::MAX);
    if !(10..=600).contains(&every_secs) || !(1..=20).contains(&max_times) {
        return Err("repeat chime must be every 10s to 10m, 1x to 20x".to_owned());
    }
    Ok(Reminder {
        every_secs,
        max_times,
    })
}

/// "#c0ff33" -> [192, 255, 51]. The short form "#f80" means "#ff8800".
fn parse_color(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#')?;
    // `collect` into `Option<Vec<_>>` gives `None` if any character isn't hex.
    let digits: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    // Matching on a slice pattern picks the right form by length.
    match digits[..] {
        [r, g, b] => Some([r * 17, g * 17, b * 17]),
        [r1, r2, g1, g2, b1, b2] => Some([r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2]),
        _ => None,
    }
}

/// "3x" -> 3.
fn parse_repeat(text: &str) -> Option<u32> {
    let count = text.strip_suffix('x').or_else(|| text.strip_suffix('X'))?;
    count.trim_end().parse().ok()
}

/// Expands repeats into a flat list of steps. `rounds` tracks which round of each
/// enclosing repeat we're in, so every step can show "Round 2/3". `outer` holds the
/// `[...]` options of those enclosing repeats.
fn unroll(
    nodes: &[Node],
    rounds: &mut Vec<Round>,
    outer: StepOptions,
    out: &mut Vec<Step>,
) -> Result<(), ParseError> {
    for node in nodes {
        match node {
            Node::Step {
                label,
                duration,
                options,
            } => {
                if out.len() >= MAX_STEPS {
                    return Err(ParseError::whole_file(format!(
                        "too many intervals once repeats are counted (over {MAX_STEPS})"
                    )));
                }
                out.push(Step {
                    label: label.clone(),
                    duration: *duration,
                    mode: mode_for_label(label),
                    color: None, // filled in by `parse`, from the settings block
                    options: options.within(outer),
                    rounds: rounds.clone(),
                });
            }
            Node::Repeat {
                times,
                options,
                body,
            } => {
                for current in 1..=*times {
                    rounds.push(Round {
                        current,
                        total: *times,
                    });
                    unroll(body, rounds, options.within(outer), out)?;
                    rounds.pop();
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    /// The schedule as (label, minutes) pairs, which is easy to compare in tests.
    fn summary(text: &str) -> Vec<(String, u64)> {
        let schedule = Schedule::parse(text).unwrap();
        (0..schedule.step_count())
            .map(|i| {
                let step = schedule.step(i);
                (step.label.clone(), step.duration.as_secs() / 60)
            })
            .collect()
    }

    fn pairs(list: &[(&str, u64)]) -> Vec<(String, u64)> {
        list.iter().map(|(l, m)| (l.to_string(), *m)).collect()
    }

    fn error(text: &str) -> ParseError {
        Schedule::parse(text).unwrap_err()
    }

    #[test]
    fn flat_repeat_ends_at_blank_line() {
        let text = "3x\n- 30m sit\n- 15m stand\n\n- 15m break\n";
        let sit_stand = [("sit", 30), ("stand", 15)];
        let mut expected = sit_stand.repeat(3);
        expected.push(("break", 15));
        assert_eq!(summary(text), pairs(&expected));
    }

    #[test]
    fn indented_repeats_nest() {
        let text = "2x\n  2x\n    - 25m sit\n    - 5m stand\n  - 10m break\n- 1m end";
        assert_eq!(
            summary(text),
            pairs(&[
                ("sit", 25),
                ("stand", 5),
                ("sit", 25),
                ("stand", 5),
                ("break", 10),
                ("sit", 25),
                ("stand", 5),
                ("sit", 25),
                ("stand", 5),
                ("break", 10),
                ("end", 1),
            ])
        );
    }

    #[test]
    fn flat_repeat_inside_flat_repeat_runs_to_the_same_blank_line() {
        assert_eq!(
            summary("2x\n2x\n- 1m a\n\n- 2m b"),
            pairs(&[("a", 1), ("a", 1), ("a", 1), ("a", 1), ("b", 2)])
        );
    }

    #[test]
    fn blank_after_indented_block_still_ends_the_outer_flat_repeat() {
        assert_eq!(
            summary("2x\n3x\n  - 1m a\n\n- 2m b"),
            pairs(&[
                ("a", 1),
                ("a", 1),
                ("a", 1),
                ("a", 1),
                ("a", 1),
                ("a", 1),
                ("b", 2)
            ])
        );
    }

    #[test]
    fn blank_lines_inside_an_indented_block_are_ignored() {
        assert_eq!(
            summary("2x\n  - 1m a\n\n  - 2m b\n"),
            pairs(&[("a", 1), ("b", 2), ("a", 1), ("b", 2)])
        );
    }

    #[test]
    fn comments_are_ignored_and_dont_end_a_block() {
        assert_eq!(
            summary("# header\n2x   # twice\n- 1m a\n# note\n- 2m b # trailing\n"),
            pairs(&[("a", 1), ("b", 2), ("a", 1), ("b", 2)])
        );
    }

    #[test]
    fn rounds_are_tracked_for_nested_repeats() {
        let schedule = Schedule::parse("2x\n  3x\n    - 1m a\n- 1m b").unwrap();
        let step = schedule.step(4); // second outer round, second inner round
        assert_eq!(step.rounds_text().as_deref(), Some("Round 2/2 · 2/3"));
        assert_eq!(schedule.step(6).rounds_text(), None);
    }

    #[test]
    fn labels_pick_the_mode() {
        let schedule = Schedule::parse("- 1m Sitting\n- 1m stand up\n- 1m walk").unwrap();
        assert_eq!(schedule.step(0).mode, Mode::Sit);
        assert_eq!(schedule.step(1).mode, Mode::Stand);
        assert_eq!(schedule.step(2).mode, Mode::Break);
    }

    #[test]
    fn durations_parse() {
        assert_eq!(parse_duration("30m"), Some(30 * MIN));
        assert_eq!(parse_duration("1h30m"), Some(90 * MIN));
        assert_eq!(parse_duration("90s"), Some(Duration::from_secs(90)));
        assert_eq!(parse_duration("1M5S"), Some(Duration::from_secs(65)));
        for bad in ["30", "m", "1hm", "5x", "", "1.5h"] {
            assert_eq!(parse_duration(bad), None, "{bad}");
        }
    }

    #[test]
    fn step_wraps_around() {
        let schedule = Schedule::simple(30 * MIN, 10 * MIN);
        assert_eq!(schedule.step(2).label, "sit");
        assert_eq!(schedule.cycle_length(), 40 * MIN);
    }

    #[test]
    fn errors_name_the_line() {
        assert_eq!(error("- 30m sit\n- 15 stand").line, 2);
        assert_eq!(error("- 30m").line, 1);
        assert_eq!(error("- 0m sit").line, 1);
        assert_eq!(error("sit 30m").line, 1);
        assert_eq!(error("3x\n\n- 1m a").line, 1); // nothing to repeat
        assert_eq!(error("0x\n- 1m a").line, 1);
        assert_eq!(error("- 1m a\n  - 1m b").line, 2); // unexpected indentation
        assert_eq!(error("  - 1m a\n- 1m b").line, 2);
        assert_eq!(error("# only comments\n\n").line, 0);
    }

    #[test]
    fn huge_repeats_are_refused() {
        let err = error("1000x\n  1000x\n    - 1s a");
        assert!(err.message.contains("too many"), "{err}");
    }

    #[test]
    fn pomodoro_file_parses() {
        let schedule = Schedule::parse(POMODORO).unwrap();
        let labels: Vec<&str> = (0..schedule.step_count())
            .map(|i| schedule.step(i).label.as_str())
            .collect();
        assert_eq!(
            labels,
            [
                "sit",
                "stand",
                "sit",
                "stand",
                "sit",
                "stand",
                "sit",
                "long break"
            ]
        );
        assert_eq!(schedule.settings().auto_start, Some(true));
        assert_eq!(schedule.step(0).options.auto_start, Some(false)); // focus waits
        assert_eq!(schedule.step(6).options.auto_start, Some(false));
        assert_eq!(schedule.step(7).color, Some([0xb0, 0x7c, 0xff]));
    }

    #[test]
    fn settings_block_colors_matching_labels() {
        let text = "# my schedule\n\n*****\nLong  Break: #c0ff33\nwalk: #f80\n\
                    unused: #000000\n*****\n\n- 1m sit\n- 1m long break\n- 1m WALK";
        let schedule = Schedule::parse(text).unwrap();
        assert_eq!(schedule.step(0).color, None);
        assert_eq!(schedule.step(1).color, Some([0xc0, 0xff, 0x33]));
        assert_eq!(schedule.step(2).color, Some([0xff, 0x88, 0x00]));
        assert_eq!(schedule.step_count(), 3);
    }

    #[test]
    fn settings_block_errors_name_the_line() {
        assert_eq!(error("*****\nbreak: #abc\n- 1m a").line, 1); // never closed
        assert_eq!(error("*****\nbreak: c0ff33\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\nbreak: #12345\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\nbreak #abc\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\na: #abc\nA: #def\n*****\n- 1m a").line, 3);
        assert_eq!(error("- 1m a\n*****\nb: #abc\n*****").line, 2); // not at the top
    }

    #[test]
    fn settings_block_sets_schedule_settings() {
        let text = "*****\nAlerts: Sound Only\nstart next: wait\nrepeat chime: 1m, 3x\n\
                    walk: #f80\n*****\n- 1m walk";
        let schedule = Schedule::parse(text).unwrap();
        assert_eq!(
            schedule.settings(),
            &ScheduleSettings {
                alerts: Some(AlertStyle::SoundOnly),
                auto_start: Some(false),
                reminder: Some(Some(Reminder {
                    every_secs: 60,
                    max_times: 3
                })),
                flash: None,
            }
        );
        let flash = Schedule::parse("*****\nflash: 15s\n*****\n- 1m a").unwrap();
        assert_eq!(flash.settings().flash, Some(Duration::from_secs(15)));
        let off = Schedule::parse("*****\nrepeat chime: off\n*****\n- 1m a").unwrap();
        assert_eq!(off.settings().reminder, Some(None));
        let none = Schedule::parse("- 1m a").unwrap();
        assert_eq!(none.settings(), &ScheduleSettings::default());
    }

    #[test]
    fn bad_settings_name_the_line() {
        assert_eq!(error("*****\nalerts: loud\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\nstart next: soon\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\nrepeat chime: 5s, 3x\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\nrepeat chime: 60s\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\nvolume: 0.5\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\nflash: 15\n*****\n- 1m a").line, 2);
        assert_eq!(error("*****\nflash: 11m\n*****\n- 1m a").line, 2);
        assert_eq!(
            error("*****\nalerts: all\nalerts: silent\n*****\n- 1m a").line,
            3
        );
    }

    #[test]
    fn options_attach_to_steps_and_repeats() {
        let text = "2x [no sound]\n  - 1m sit [no flash, wait]\n  - 1m stand\n- 1m walk [silent]";
        let schedule = Schedule::parse(text).unwrap();
        let sit = schedule.step(0);
        assert_eq!(sit.label, "sit");
        assert_eq!(
            sit.options,
            StepOptions {
                no_sound: true, // from the repeat
                no_flash: true,
                no_notification: false,
                auto_start: Some(false),
            }
        );
        assert_eq!(
            schedule.step(1).options,
            StepOptions {
                no_sound: true,
                ..StepOptions::default()
            }
        );
        let walk = schedule.step(4);
        assert_eq!(walk.label, "walk");
        assert!(walk.options.no_sound && walk.options.no_flash && walk.options.no_notification);
    }

    #[test]
    fn inner_auto_or_wait_beats_the_repeats() {
        let schedule = Schedule::parse("2x [wait]\n  - 1m a [auto]\n  - 1m b").unwrap();
        assert_eq!(schedule.step(0).options.auto_start, Some(true));
        assert_eq!(schedule.step(1).options.auto_start, Some(false));
    }

    #[test]
    fn bad_options_name_the_line() {
        assert_eq!(error("- 1m a\n- 1m b [loud]").line, 2);
        assert_eq!(error("- 1m a [auto, wait]").line, 1);
        assert_eq!(error("- 1m a]").line, 1);
        assert_eq!(error("- 1m [silent]").line, 1); // no label left
    }

    #[test]
    fn hash_starts_a_comment_only_before_a_space_or_at_line_start() {
        assert_eq!(strip_comment("- 5m break #2"), "- 5m break #2");
        assert_eq!(strip_comment("walk: #f80"), "walk: #f80");
        assert_eq!(strip_comment("- 5m break # note"), "- 5m break ");
        assert_eq!(strip_comment("- 5m break #"), "- 5m break ");
        assert_eq!(strip_comment("  #not-spaced"), "  ");
    }

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#C0FF33"), Some([192, 255, 51]));
        assert_eq!(parse_color("#fff"), Some([255, 255, 255]));
        for bad in ["c0ff33", "#c0ff3", "#c0ff3g", "#", ""] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
    }

    #[test]
    fn describes_durations_in_words() {
        assert_eq!(describe_duration(15 * MIN), "15 minutes");
        assert_eq!(describe_duration(61 * MIN), "1 hour 1 minute");
        assert_eq!(
            describe_duration(Duration::from_secs(90)),
            "1 minute 30 seconds"
        );
    }
}
