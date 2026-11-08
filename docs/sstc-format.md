# The `.sstc` schedule format

A `.sstc` (**S**it-**S**tand **T**ime **C**onfig) file describes a routine for the
Sit-Stand Timer: which intervals to time, in what order, how often to repeat
them, and how each change is announced. This page is the complete reference.

```
*****
start next: auto
long break: #b07cff
*****

3x
- 25m sit [wait]       # focus
- 5m stand             # short break

- 25m sit [wait]
- 20m long break
```

## Contents

1. [Using schedule files](#using-schedule-files)
2. [Intervals](#intervals)
3. [Repeats](#repeats)
4. [Comments and blank lines](#comments-and-blank-lines)
5. [Options in brackets](#options-in-brackets)
6. [The settings block](#the-settings-block)
7. [What you see while it runs](#what-you-see-while-it-runs)
8. [Errors](#errors)
9. [Limits](#limits)
10. [Examples](#examples)
11. [Compatibility](#compatibility)

## Using schedule files

- Schedules live in one folder, `%APPDATA%\SitStand\config\schedules` by default.
  To use another folder, open **Settings → Schedule → Other folder** and click **Browse…** (or type its path). **Default** goes back to this folder.
- The default folder is created on the first run, with a `Pomodoro.sstc` in it to start from.
- Pick a schedule from the **Schedule** dropdown in the settings, or from the tray
  menu → **Schedule**. **Plain sit/stand** turns schedules off.
- A file shows up under its name without `.sstc`, so `Deep Work.sstc` is listed as
  **Deep Work**. Spaces and capitals are fine.
- New or deleted files appear in the lists within a few seconds. After *editing*
  the schedule in use, press **Reload** (or pick it again) to read it again.
- Picking or reloading a changed schedule starts it from its first interval.
- Files are plain text (UTF-8). Any text editor works; **Open folder** shows the
  folder in Explorer.

## Intervals

An interval is a line starting with `-`, then a length, then a label:

```
- 30m sit
- 90s stand
- 1h30m lunch break
```

**Lengths** are numbers with a unit: `h` (hours), `m` (minutes) or `s` (seconds),
upper or lower case. Combine them with no spaces in between: `1h30m`, `2m30s`.
Every number needs a unit (`30` alone is an error), decimals aren't allowed (write
`1h30m`, not `1.5h`), and an interval must be 1 second to 24 hours long.

**Labels** are everything after the length, spaces included, up to any
[options](#options-in-brackets) or [comment](#comments-and-blank-lines). A label is
required. It's shown in capitals on the widget (`- 5m stand up` shows **STAND UP**)
and used as-is in notifications.

A label's **first word** decides its kind, ignoring case. The first word ends at
anything that isn't a letter, so `stand-up` counts as `stand`.

| First word | Kind | Default color | Chime |
| --- | --- | --- | --- |
| `sit`, `sits`, `sitting` (sit, Sit down, sitting work...) | sit | the **Sit** color in settings | sit chime |
| `stand`, `stands`, `standing` (stand, stand up, stand-up...) | stand | the **Stand** color | stand chime |
| anything else (break, walk, lunch, situps, standup meeting...) | break | the **Break** color | stand chime |

A label can have [its own color](#label-colors) instead.

## Repeats

`<N>x` repeats the lines after it N times (1 to 1000). `3x`, `3X` and `3 x` all work.
There are two ways to say which lines are repeated.

**Flat style: up to the next blank line.** If the line after `Nx` isn't indented
more than the `Nx` itself, the repeat holds the following lines up to the next
blank line (or the end of the file):

```
3x
- 30m sit
- 15m stand

- 15m break        # after the blank line: runs once, after all 3 rounds
```

**Indented style: everything indented under it.** If the line after `Nx` is
indented more, the repeat holds every line indented under it. This is how repeats
nest:

```
4x
  2x
    - 25m sit
    - 5m stand
  - 10m break      # inside 4x, after the 2x
- 30m lunch        # back at the left edge: after all 4 rounds
```

**Indentation rules:**

- Any amount of extra indentation counts: one space is enough. A tab counts as
  4 spaces, so a tab and four spaces are the same depth. Mixing them can make lines
  that look aligned come out different, so pick one.
- The first line under an `Nx` sets the depth for that block. Every other line in
  the block must be at exactly that depth. Less indentation ends the block (and
  must land exactly on an outer level). More indentation is only allowed right
  after another `Nx`.
- The first line of the file sets the left edge; it doesn't have to be column 0.

**Blank lines:**

- In flat style, a blank line ends the repeat. A flat repeat inside a flat repeat
  runs to the same blank line, which ends both:

  ```
  2x
  3x
  - 1m a           # a, a, a  twice over: 6 times
  
  - 2m b           # once
  ```
- Inside an indented block, blank lines are ignored, except that a blank line
  right before the block ends still counts. So a blank line after an indented
  block also ends a flat repeat around it.

When the last interval ends, the schedule **starts over from the top**.

## Comments and blank lines

`#` starts a comment, to the end of the line, when it is the first thing on the
line or is followed by a space:

```
# This whole line is a comment.
- 25m sit # this part is a comment
- 5m break #2      # the label is "break #2": "#2" has no space after the #
```

A line that is only a comment is ignored completely: it doesn't count as a blank
line, so it never ends a repeat. This is also why colors like `#c0ff33` work.

## Options in brackets

Put `[...]` at the end of an interval line to change what happens **when that
interval starts**. Separate several options with commas. Case doesn't matter.

| Option | Effect when the interval starts |
| --- | --- |
| `silent` | No chime, no flash, no notification |
| `no sound` | No chime (and so no repeat chime) |
| `no flash` | The widget doesn't flash |
| `no notification` | No desktop notification |
| `wait` | Wait for a click on the widget before it starts counting |
| `auto` | Start counting by itself |

```
- 2m move [silent]
- 8m stand [no sound, wait]
```

**On a repeat line**, options apply to everything inside the repeat:

```
4x [no notification]
  - 20m sit
  - 8m stand [no sound]    # no notification (from 4x) and no sound
```

**How they combine:**

- The "no ..." options add up: inner lines keep everything the repeats around
  them turned off, and can turn off more.
- For `wait`/`auto`, the innermost one wins: a `[auto]` interval inside a
  `3x [wait]` starts by itself.
- `wait` and `auto` can't both be on one line.
- Options only ever turn alert parts **off**. They can't add a chime when the
  alert style is "Visual only", for example.
- Because `[...]` at the end of a line is always read as options, a label can't
  end with brackets.

## The settings block

The file can start with a settings block between two lines of `*****` (three or
more `*`, nothing else on the line). Comments and blank lines may come before it,
but nothing else. Inside, each line is `name: value`:

```
*****
alerts: sound only
start next: auto
repeat chime: 60s, 3x
flash: 15s
long break: #b07cff
walk: #f80
*****
```

- A value starting with `#` is a [label color](#label-colors). Any other value is a
  [schedule setting](#schedule-settings).
- Names and values ignore case and extra spaces.
- Each name may appear only once.
- Blank lines and comments are allowed inside the block.

### Label colors

`label: #rrggbb` gives every interval with that label its own color. The short form
`#rgb` also works (`#f80` is `#ff8800`). The color is used for the ring, the
title, the border, the compact view's bar, the flash and the tray icon.

- Labels match ignoring case and extra spaces: `Long  Break: #b07cff` colors
  `- 20m long break`.
- The color replaces only the color. The chime still comes from how the label
  starts.
- You can recolor `sit` or `stand` this way too.
- A color for a label the schedule doesn't use is ignored, so you can comment out
  an interval without editing the block. (A typo in a label is ignored the same way.)
- Labels without a color here use the Sit, Stand or Break color from the settings.

### Schedule settings

These replace the matching app settings while this schedule is loaded. The
settings panel then shows them as "(set by schedule)" instead of their controls.
Anything you leave out uses the app setting.

| Setting | Values | Replaces the app setting |
| --- | --- | --- |
| `alerts` | `all`, `visual only`, `sound only`, `silent` | **Alerts** dropdown |
| `start next` | `auto` (start by itself) or `wait` (for a click) | **Start next interval by itself** |
| `repeat chime` | `<length>, <N>x`, e.g. `60s, 3x`; or `off` | **Repeat chime until clicked** |
| `flash` | a length from `1s` to `10m`, e.g. `15s` | **Flash for** |

The alert styles mean:

| Style | Chime | Flash | Notification |
| --- | --- | --- | --- |
| `all` | yes | yes | yes |
| `visual only` | | yes | |
| `sound only` | yes | | |
| `silent` | | | |

`repeat chime: 60s, 3x` plays the chime again every 60 seconds, at most 3 more
times, until the widget is clicked. The interval must be 10 seconds to 10 minutes,
and the count 1 to 20. It only follows an alert that chimed.

`flash: 15s` makes the widget flash for 15 seconds when an interval starts (the app
default is 60). Clicking the widget stops it sooner, and a flash never lasts longer
than the interval it announces. It only matters when the alert style flashes.

Volume, mute, opacity, compact mode, click-through and the window position are
not schedule settings: they always stay app-wide. Mute silences every schedule.

### Which setting wins

- **Starting the next interval:** the interval's own `[wait]`/`[auto]` (the
  innermost one), then the file's `start next`, then the app setting.
- **Alerts:** the file's `alerts` if set, otherwise the app's, minus whatever the
  starting interval's options turn off. The app's **Mute** then silences any chime.
- **Repeat chime:** the file's `repeat chime` if set, otherwise the app's.
- **Flash length:** the file's `flash` if set, otherwise the app's, but never longer
  than the interval.

## What you see while it runs

- The widget's title is the current label in capitals, cut short with "…" if it's
  too long to fit.
- Inside a repeat, the status line shows the round, outermost repeat first:
  `Round 2/3`, or `Round 2/4 · 1/2` for nested repeats.
- When an interval starts, its alert can chime, flash the widget and show a desktop
  notification. The notification's title is the label ("Long break"); plain `sit`
  and `stand` get "Time to sit!" and "Time to stand!". The text says how long it
  lasts and what comes next: "Long break for 20 minutes, then sit."
- **Skip** goes to the next interval. Tray → **Restart schedule** goes back to the
  first. Tray → **Restart interval** restarts the current one.

## Errors

If a file has a mistake, the settings panel shows the file name, the line number
and what's wrong, for example:

```
Posture.sstc, line 12: `15` isn't a length: use e.g. 90s, 30m, 1h or 1h30m
```

The widget's status line says "Schedule error", and the timer runs plain sit/stand
until the file is fixed and reloaded. Common causes:

- A length without a unit (`- 15 stand`), or an interval without a label.
- `Nx` with nothing under it (for example, followed straight away by a blank line).
- A line indented differently from the others in its block.
- A settings block that isn't at the top, or isn't closed with a second `*****`.
- An unknown setting, option or value, or a name set twice in the block.

## Limits

| What | Limit |
| --- | --- |
| Interval length | 1 second to 24 hours |
| Repeat count | 1 to 1000 |
| Intervals in total, after repeats are counted | 10,000 |
| Repeat chime | every 10 seconds to 10 minutes, 1 to 20 times |
| Flash length | 1 second to 10 minutes (and at most the interval's length) |

## Examples

**Pomodoro.** Focus sessions start when you click; breaks start by themselves.

```
*****
start next: auto
long break: #b07cff
*****

3x
- 25m sit [wait]       # focus
- 5m stand             # short break

- 25m sit [wait]       # 4th focus
- 20m long break
```

**Posture (20-8-2).** Change position every few minutes; only the walk gets a desktop
notification.

```
*****
start next: auto
flash: 15s
move: #2ec4b6
walk: #ff7a59
*****

4x [no notification]
  - 20m sit
  - 8m stand
  - 2m move     # walk around, stretch, roll your shoulders
- 10m walk      # the bigger break, after 2 hours
```

**A workday.** A morning and an afternoon of sit/stand hours around lunch, which
waits until you're ready, then a quiet wrap-up. The repeat chime makes sure you
notice each change.

```
*****
alerts: all
repeat chime: 2m, 2x
lunch: #ff7a59
*****

3x
  - 40m sit
  - 20m stand
- 45m lunch [wait]       # waits for your click, then counts down
3x
  - 40m sit
  - 20m stand
- 5m wrap up [no sound]
```

## Compatibility

The format only grows. Later versions may add options, settings or syntax, but a
file that works today keeps working, and keeps meaning the same thing, in every
later version. So files don't carry a version number.

The other direction isn't promised: a file using something new shows an error
("unknown option", for example) in an older version of the app.
