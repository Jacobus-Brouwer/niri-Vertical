use smithay::utils::{Logical, Point, Size};

/// The scrolling axis for a workspace's tiling layout.
///
/// `Horizontal` is current niri behaviour: columns arranged left→right, view scrolls on X.
/// `Vertical` is the new mode: rows arranged top→bottom, view scrolls on Y. In Vertical mode
/// each "column" contains exactly one tile that spans the full working-area width (cross-axis).
///
/// Terminology used throughout layout code:
/// - **main axis** — the scroll direction (X for `Horizontal`, Y for `Vertical`).
/// - **cross axis** — perpendicular to scrolling (Y for `Horizontal`, X for `Vertical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    /// Columns arranged left→right, view scrolls on X. Current niri behaviour.
    Horizontal,
    /// Rows arranged top→bottom, view scrolls on Y. Each row is one window wide.
    Vertical,
}

impl ScrollAxis {
    /// Extract the main-axis (scroll-axis) dimension from a size.
    ///
    /// `Horizontal` → `size.w`; `Vertical` → `size.h`.
    ///
    /// Replaces direct `.w` accesses at the column-width call sites:
    /// `scrolling.rs:4401–4412` (`resolve_column_width`) and the gesture
    /// normalization at `scrolling.rs:3076`.
    pub fn main_size(self, size: Size<f64, Logical>) -> f64 {
        match self {
            ScrollAxis::Horizontal => size.w,
            ScrollAxis::Vertical => size.h,
        }
    }

    /// Extract the cross-axis dimension from a size.
    ///
    /// `Horizontal` → `size.h`; `Vertical` → `size.w`.
    ///
    /// Replaces `.h` accesses for the tile-height budget:
    /// `scrolling.rs:4490` (`max_tile_height = working_size.h - gaps * 2.`).
    pub fn cross_size(self, size: Size<f64, Logical>) -> f64 {
        match self {
            ScrollAxis::Horizontal => size.h,
            ScrollAxis::Vertical => size.w,
        }
    }

    /// Build a `Size<f64, Logical>` from (main, cross) components.
    ///
    /// `Horizontal`: `Size { w: main, h: cross }`.
    /// `Vertical`:   `Size { w: cross, h: main }`.
    pub fn make_size(self, main: f64, cross: f64) -> Size<f64, Logical> {
        match self {
            ScrollAxis::Horizontal => Size::from((main, cross)),
            ScrollAxis::Vertical => Size::from((cross, main)),
        }
    }

    /// Extract the main-axis coordinate from a point.
    ///
    /// `Horizontal` → `point.x`; `Vertical` → `point.y`.
    ///
    /// Replaces `.x` in column-position accumulation at `scrolling.rs:2304–2323`
    /// (`column_xs`, `column_x`).
    pub fn main_coord(self, point: Point<f64, Logical>) -> f64 {
        match self {
            ScrollAxis::Horizontal => point.x,
            ScrollAxis::Vertical => point.y,
        }
    }

    /// Extract the cross-axis coordinate from a point.
    ///
    /// `Horizontal` → `point.y`; `Vertical` → `point.x`.
    ///
    /// Replaces `.y` in tile-origin computation at `scrolling.rs:5169–5190`
    /// (`tiles_origin`).
    pub fn cross_coord(self, point: Point<f64, Logical>) -> f64 {
        match self {
            ScrollAxis::Horizontal => point.y,
            ScrollAxis::Vertical => point.x,
        }
    }

    /// Build a `Point<f64, Logical>` from (main, cross) components.
    ///
    /// `Horizontal`: `Point { x: main, y: cross }`.
    /// `Vertical`:   `Point { x: cross, y: main }`.
    pub fn make_point(self, main: f64, cross: f64) -> Point<f64, Logical> {
        match self {
            ScrollAxis::Horizontal => Point::from((main, cross)),
            ScrollAxis::Vertical => Point::from((cross, main)),
        }
    }

    /// Set the main-axis coordinate of a point in place.
    ///
    /// `Horizontal` sets `point.x`; `Vertical` sets `point.y`.
    ///
    /// Replaces `offset.x += ...` in `Column::render_offset()` at
    /// `scrolling.rs:4160` and `animate_move_from` at `scrolling.rs:4166–4188`.
    pub fn set_main(self, point: &mut Point<f64, Logical>, v: f64) {
        match self {
            ScrollAxis::Horizontal => point.x = v,
            ScrollAxis::Vertical => point.y = v,
        }
    }

    /// Set the cross-axis coordinate of a point in place.
    ///
    /// `Horizontal` sets `point.y`; `Vertical` sets `point.x`.
    ///
    /// Replaces `origin.y += ...` assignments in `tiles_origin()` at
    /// `scrolling.rs:5181` and `tile_offsets_iter()` at `scrolling.rs:5234`.
    pub fn set_cross(self, point: &mut Point<f64, Logical>, v: f64) {
        match self {
            ScrollAxis::Horizontal => point.y = v,
            ScrollAxis::Vertical => point.x = v,
        }
    }

    /// Build the view-offset `Point` from a signed scroll scalar.
    ///
    /// The result is added to tile positions to shift visible content: a positive `offset`
    /// means content has scrolled forward by that many logical pixels, so tiles move backward.
    ///
    /// `Horizontal` → `(-offset, 0.)` (shift left on screen).
    /// `Vertical`   → `(0., -offset)` (shift up on screen).
    ///
    /// Replaces `Point::from((-self.view_pos(), 0.))` at `scrolling.rs:2381` and `:2402`
    /// (`tiles_with_render_positions` and `tiles_with_render_positions_mut`).
    pub fn view_offset_point(self, offset: f64) -> Point<f64, Logical> {
        match self {
            ScrollAxis::Horizontal => Point::from((-offset, 0.)),
            ScrollAxis::Vertical => Point::from((0., -offset)),
        }
    }

    /// Build the column-origin `Point` from a main-axis position scalar.
    ///
    /// The cross-axis component is always zero here: columns are positioned only along the
    /// main axis. Their cross-axis placement is handled by the working-area offset applied
    /// separately in `tiles_origin()`.
    ///
    /// `Horizontal` → `(main_pos, 0.)`.
    /// `Vertical`   → `(0., main_pos)`.
    ///
    /// Replaces `Point::from((col_x, 0.))` at `scrolling.rs:2384` and `:2405`
    /// (`tiles_with_render_positions` and `tiles_with_render_positions_mut`).
    pub fn column_origin_point(self, main_pos: f64) -> Point<f64, Logical> {
        match self {
            ScrollAxis::Horizontal => Point::from((main_pos, 0.)),
            ScrollAxis::Vertical => Point::from((0., main_pos)),
        }
    }

    /// The gesture delta that drives within-workspace scroll.
    ///
    /// `Horizontal` → `delta_x` (3-finger left/right swipe scrolls columns).
    /// `Vertical`   → `delta_y` (3-finger up/down swipe scrolls rows).
    ///
    /// Replaces the hardcoded `delta_x` parameter in `view_offset_gesture_update` at
    /// `scrolling.rs:3059` and the routing call at `input/mod.rs:~3910`.
    pub fn scroll_gesture_delta(self, delta_x: f64, delta_y: f64) -> f64 {
        match self {
            ScrollAxis::Horizontal => delta_x,
            ScrollAxis::Vertical => delta_y,
        }
    }

    /// The gesture delta that drives workspace switching.
    ///
    /// Always the axis opposite to `scroll_gesture_delta`.
    ///
    /// `Horizontal` → `delta_y` (3-finger up/down swipe switches workspaces).
    /// `Vertical`   → `delta_x` (3-finger left/right swipe switches workspaces).
    ///
    /// Replaces the hardcoded `delta_y` routing at `input/mod.rs:~3885`.
    pub fn switch_gesture_delta(self, delta_x: f64, delta_y: f64) -> f64 {
        match self {
            ScrollAxis::Horizontal => delta_y,
            ScrollAxis::Vertical => delta_x,
        }
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point, Size};

    use super::ScrollAxis;

    #[test]
    fn horizontal_main_is_width_cross_is_height() {
        let size = Size::<f64, Logical>::from((1280., 720.));
        assert_eq!(ScrollAxis::Horizontal.main_size(size), size.w);
        assert_eq!(ScrollAxis::Horizontal.cross_size(size), size.h);
    }

    #[test]
    fn vertical_main_is_height_cross_is_width() {
        let size = Size::<f64, Logical>::from((1280., 720.));
        assert_eq!(ScrollAxis::Vertical.main_size(size), size.h);
        assert_eq!(ScrollAxis::Vertical.cross_size(size), size.w);
    }

    #[test]
    fn make_size_round_trip_horizontal() {
        let (main, cross) = (800., 600.);
        let size = ScrollAxis::Horizontal.make_size(main, cross);
        assert_eq!(ScrollAxis::Horizontal.main_size(size), main);
        assert_eq!(ScrollAxis::Horizontal.cross_size(size), cross);
    }

    #[test]
    fn make_size_round_trip_vertical() {
        let (main, cross) = (800., 600.);
        let size = ScrollAxis::Vertical.make_size(main, cross);
        assert_eq!(ScrollAxis::Vertical.main_size(size), main);
        assert_eq!(ScrollAxis::Vertical.cross_size(size), cross);
    }

    #[test]
    fn make_point_round_trip_horizontal() {
        let (main, cross) = (100., 50.);
        let point = ScrollAxis::Horizontal.make_point(main, cross);
        assert_eq!(ScrollAxis::Horizontal.main_coord(point), main);
        assert_eq!(ScrollAxis::Horizontal.cross_coord(point), cross);
    }

    #[test]
    fn make_point_round_trip_vertical() {
        let (main, cross) = (100., 50.);
        let point = ScrollAxis::Vertical.make_point(main, cross);
        assert_eq!(ScrollAxis::Vertical.main_coord(point), main);
        assert_eq!(ScrollAxis::Vertical.cross_coord(point), cross);
    }

    #[test]
    fn view_offset_point_horizontal() {
        let p = ScrollAxis::Horizontal.view_offset_point(42.);
        assert_eq!(p, Point::<f64, Logical>::from((-42., 0.)));
    }

    #[test]
    fn view_offset_point_vertical() {
        let p = ScrollAxis::Vertical.view_offset_point(42.);
        assert_eq!(p, Point::<f64, Logical>::from((0., -42.)));
    }

    #[test]
    fn gesture_routing_horizontal() {
        assert_eq!(ScrollAxis::Horizontal.scroll_gesture_delta(10., 5.), 10.);
        assert_eq!(ScrollAxis::Horizontal.switch_gesture_delta(10., 5.), 5.);
    }

    #[test]
    fn gesture_routing_vertical() {
        assert_eq!(ScrollAxis::Vertical.scroll_gesture_delta(10., 5.), 5.);
        assert_eq!(ScrollAxis::Vertical.switch_gesture_delta(10., 5.), 10.);
    }

    #[test]
    fn scroll_and_switch_are_opposite_axes() {
        for dx in [0., 1., -7.5] {
            for dy in [0., 2., -3.] {
                for axis in [ScrollAxis::Horizontal, ScrollAxis::Vertical] {
                    let scroll = axis.scroll_gesture_delta(dx, dy);
                    let switch = axis.switch_gesture_delta(dx, dy);
                    // One must be dx and the other dy — they are never the same component
                    // unless dx == dy (degenerate case).
                    if dx != dy {
                        assert_ne!(scroll, switch, "{axis:?}: scroll and switch must differ");
                    }
                }
            }
        }
    }
}
