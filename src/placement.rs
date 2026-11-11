//! Where the window goes when it changes size (compact ↔ normal, settings open/close),
//! so that it stays on screen in any corner of any monitor.
//!
//! Pure geometry, no window code: the caller passes the window's current rect and
//! the work area (the monitor minus the taskbar) of the monitor it's on, both in
//! the same units, and gets back the rect to move the window to.

use eframe::egui::{Rect, Vec2, pos2};

/// The rect for a window that is changing to `new_size`.
///
/// The window grows away from the nearest screen edges: on the right half of the
/// monitor its right edge stays put (it grows leftwards), on the bottom half its
/// bottom edge stays put (it grows upwards). Shrinking back uses the same rule,
/// so a round trip returns the window to where it started. The result is then
/// pushed fully inside `work_area`, in case the new size still reaches past it.
pub fn place(old: Rect, new_size: Vec2, work_area: Rect) -> Rect {
    let center = work_area.center();
    let x = if old.center().x > center.x {
        old.max.x - new_size.x
    } else {
        old.min.x
    };
    let y = if old.center().y > center.y {
        old.max.y - new_size.y
    } else {
        old.min.y
    };
    keep_inside(Rect::from_min_size(pos2(x, y), new_size), work_area)
}

/// Move `rect` (without resizing it) so it lies inside `area`. A rect bigger than
/// the area is lined up with the area's top-left corner, so the top of the window,
/// where it can be dragged from, is always reachable.
pub fn keep_inside(rect: Rect, area: Rect) -> Rect {
    // `f32::clamp` panics if min > max, so the upper limit is never let below the lower.
    let x = rect
        .min
        .x
        .clamp(area.min.x, (area.max.x - rect.width()).max(area.min.x));
    let y = rect
        .min
        .y
        .clamp(area.min.y, (area.max.y - rect.height()).max(area.min.y));
    Rect::from_min_size(pos2(x, y), rect.size())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    /// A 1920×1040 work area (a 1080p screen with a 40px taskbar).
    fn screen() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1040.0))
    }

    fn at(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    #[test]
    fn top_left_grows_right_and_down() {
        let old = at(10.0, 10.0, 120.0, 44.0);
        assert_eq!(
            place(old, vec2(140.0, 140.0), screen()),
            at(10.0, 10.0, 140.0, 140.0)
        );
    }

    #[test]
    fn top_right_grows_left() {
        let old = at(1790.0, 10.0, 120.0, 44.0);
        // Right edge stays at 1910.
        assert_eq!(
            place(old, vec2(140.0, 140.0), screen()),
            at(1770.0, 10.0, 140.0, 140.0)
        );
    }

    #[test]
    fn bottom_left_grows_up() {
        let old = at(10.0, 986.0, 120.0, 44.0);
        // Bottom edge stays at 1030.
        assert_eq!(
            place(old, vec2(140.0, 140.0), screen()),
            at(10.0, 890.0, 140.0, 140.0)
        );
    }

    #[test]
    fn bottom_right_grows_up_and_left() {
        let old = at(1770.0, 890.0, 140.0, 140.0);
        assert_eq!(
            place(old, vec2(220.0, 638.0), screen()),
            at(1690.0, 392.0, 220.0, 638.0)
        );
    }

    #[test]
    fn round_trip_returns_to_the_start() {
        for old in [
            at(10.0, 10.0, 120.0, 44.0),
            at(1790.0, 10.0, 120.0, 44.0),
            at(10.0, 986.0, 120.0, 44.0),
            at(1790.0, 986.0, 120.0, 44.0),
        ] {
            let big = place(old, vec2(140.0, 140.0), screen());
            assert_eq!(place(big, old.size(), screen()), old, "{old:?}");
        }
    }

    #[test]
    fn too_tall_to_fit_is_pulled_on_screen() {
        // A short screen: the settings panel can't grow up from the middle-bottom
        // without passing the top, so it's pushed down to start at the top.
        let short = at(0.0, 0.0, 1366.0, 728.0);
        let old = at(1216.0, 400.0, 140.0, 140.0);
        assert_eq!(
            place(old, vec2(220.0, 638.0), short),
            at(1136.0, 0.0, 220.0, 638.0)
        );
    }

    #[test]
    fn second_monitor_to_the_left_uses_its_own_coordinates() {
        // A monitor left of the main one has negative x.
        let left = at(-1920.0, 0.0, 1920.0, 1040.0);
        let old = at(-130.0, 10.0, 120.0, 44.0); // top-right corner of that monitor
        assert_eq!(
            place(old, vec2(140.0, 140.0), left),
            at(-150.0, 10.0, 140.0, 140.0)
        );
    }

    #[test]
    fn keep_inside_rescues_an_off_screen_window() {
        let lost = at(4000.0, -500.0, 140.0, 140.0);
        assert_eq!(keep_inside(lost, screen()), at(1780.0, 0.0, 140.0, 140.0));
    }

    #[test]
    fn keep_inside_leaves_a_visible_window_alone() {
        let fine = at(300.0, 300.0, 140.0, 140.0);
        assert_eq!(keep_inside(fine, screen()), fine);
    }
}
