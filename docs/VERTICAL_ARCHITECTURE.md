# Vertical Scrolling Architecture

**Status**: Hours 1–4. Sections 1–3 are unchanged research. Section 4 reflects the Hour 3–4
design decisions (runtime enum, not trait). Phase 0 is complete. All `file:line` references
are to the `vertical` branch in this repo.

---

## Decisions Locked

These decisions are final for this fork. Do not reopen them without a compelling technical reason.

| # | Decision | Summary |
|---|---|---|
| 1 | **Runtime enum** | `ScrollAxis` is a plain enum with inherent methods. No type parameter on `ScrollingSpace`, `Column`, `Workspace`, `Monitor`, or `Layout`. |
| 2 | **Vertical-native semantics** | Each "column" in Vertical mode contains exactly one tile spanning the full cross-axis. No within-column stacking. `ColumnWidth` config values are silently ignored in Vertical mode with a debug log. |
| 3 | **Gesture routing** | In Vertical mode, 3-finger Y swipe → within-workspace scroll; 3-finger X swipe → workspace switching. Symmetric rotation of horizontal behaviour. |
| 4 | **Global axis** | Config key `layout { scroll-direction "vertical"; }` (default `"horizontal"`). Per-workspace axis is out of scope. |
| 5 | **Keep `horizontal_view_movement`** | The config key name is unchanged and applies to whichever axis is the scroll axis. Add a doc comment. |
| 6 | **Both keybind families** | `left/right` and `up/down` keybind names work in both modes. Switching modes requires no keybind changes. |
| 7 | **Out of scope** | IPC contract changes, overview rendering adjustments, live mode-switching of existing workspaces. |

---

## 1. Inventory

All sites that assume horizontal is the scroll axis, grouped by file then by category.

### `src/layout/scrolling.rs` (~5 600 lines — the critical file)

#### Geometry

| Location | Symbol | What it hardcodes |
|---|---|---|
| 31 | `VIEW_GESTURE_WORKING_AREA_MOVEMENT: f64 = 1200.` | Normalization factor divides by `working_area.size.w` (width) |
| 274–277 | `enum ScrollDirection { Left, Right }` | Named exclusively for horizontal movement; will need `Up`/`Down` or a rename |
| 2294–2296 | `view_pos()` | Returns a scalar implicitly on the X axis: `column_x(active) + view_offset.current()` |
| 2298–2300 | `target_view_pos()` | Same as above for the animation target |
| 2304–2316 | `column_xs()` | Accumulates `x += data.width + gaps` — positions columns on X |
| 2319–2323 | `column_x()` | Calls `column_xs()`, returns a single X coordinate |
| 2381 | `tiles_with_render_positions()` | `let view_off = Point::from((-self.view_pos(), 0.))` — cross-axis is hardcoded zero |
| 2384 | `tiles_with_render_positions()` | `let col_off = Point::from((col_x, 0.))` — same |
| 2402 | `tiles_with_render_positions_mut()` | `let view_off = Point::from((-self.view_pos(), 0.))` — duplicates the above |
| 2405 | `tiles_with_render_positions_mut()` | `Point::from((col_x, 0.))` — same |
| 4401–4412 | `resolve_column_width()` | `(working_size.w - gaps) * proportion - gaps` — main-axis size uses `.w` |
| 4490 | `update_tile_sizes_with_transaction()` | `let max_tile_height = working_size.h - gaps * 2.` — cross-axis budget uses `.h` |
| 5169–5190 | `Column::tiles_origin()` | `origin.y += working_area.loc.y + gaps` — tiles start at working-area Y |
| 5181 | `Column::tiles_origin()` | `origin.y += self.working_area.loc.y + self.options.layout.gaps` |
| 5206–5212 | `Column::tile_offsets_iter()` | `tiles_width = max(data.size.w)` — used for cross-axis centering |
| 5228 | `Column::tile_offsets_iter()` | `pos.x += (tiles_width - data.size.w) / 2.` — X centering within column |
| 5229 | `Column::tile_offsets_iter()` | `pos.x += tiles_width - data.size.w` — left-edge resize offset on X |
| 5234 | `Column::tile_offsets_iter()` | `origin.y += data.size.h + gaps` — tiles stack vertically (cross-axis accumulation) |
| 5457–5489 | `compute_new_view_offset()` free fn | Parameter names: `cur_x`, `view_width`, `new_col_x`, `new_col_width` — pure horizontal |
| 5491–5519 | `compute_working_area()` | Adjusts `.w` for left/right struts and `.h` for top/bottom struts explicitly |
| 5532–5534 | `compute_toplevel_bounds()` | `working_area_size.w - gaps * 2.` and `working_area_size.h - gaps * 2.` — gives toplevel bounds in both axes independently; not wrong, but axis semantics matter for which is "main" |

#### Animation

| Location | Symbol | What it hardcodes |
|---|---|---|
| 684 | `animate_view_offset()` | Uses `self.options.animations.horizontal_view_movement.0` — animation config named for horizontal |
| 4156–4164 | `Column::render_offset()` | `offset.x += move_.from * move_.anim.value()` — column move animation is X-only |
| 4166–4188 | `Column::animate_move_from(from_x_offset: f64)` | Parameter named and typed for X axis |
| 4173–4188 | `Column::animate_move_from_with_config(from_x_offset: f64, ...)` | Same |

Note: `Tile<W>` already has **separate** `move_x_animation` and `move_y_animation` (tile.rs). Within a column, tiles animate vertically (`tile.animate_move_y_from_with_config` at scrolling.rs:4355). Only the **column-level** move animation (between columns) is X-axis specific.

#### Input / Gesture

| Location | Symbol | What it hardcodes |
|---|---|---|
| 3059–3085 | `view_offset_gesture_update(delta_x: f64, ...)` | Parameter is `delta_x`; normalization: `working_area.size.w / VIEW_GESTURE_WORKING_AREA_MOVEMENT` |
| 3073 | inside above | `gesture.tracker.push(delta_x, timestamp)` — X delta fed to tracker |
| 3076 | inside above | `self.working_area.size.w / VIEW_GESTURE_WORKING_AREA_MOVEMENT` — width normalization |

#### Popup / Hints

| Location | Symbol | What it hardcodes |
|---|---|---|
| insert_hint_area (~2436–2541) | `insert_hint_area()` | New-column hint: `size.h = working_area.size.h - gaps*2`; offset: `loc.x -= self.view_pos()` |
| popup_target_rect (~2562–2582) | `popup_target_rect()` | Uses window width for horizontal constraint; uses `parent_area.size.h` for vertical constraint |

---

### `src/layout/monitor.rs`

The monitor layer is responsible for workspace-switching, which is **always vertical regardless of
the within-workspace scroll axis**. That said, several items implicitly constrain the design:

#### Geometry

| Location | Symbol | What it hardcodes |
|---|---|---|
| `workspaces_render_geo()` | workspace Y positioning | `first_ws_y = -workspace_render_idx * ws_height_with_gap`; `y = first_ws_y + idx * ws_height_with_gap` — workspaces stack on Y |
| `workspace_gap()` | gap size | `self.view_size.h * 0.1 * zoom` — gap proportional to **height** |
| `render_workspaces()` | crop height | `height = (view_size.h * scale).ceil()` — crops output to view height |
| `workspace_under()` | hit-test | Checks `geo.loc.y <= pos.y < geo.loc.y + geo.size.h` — Y-based |
| insert hint between workspaces | Y positioning | Vertical Y arithmetic |

#### Gesture

| Location | Symbol | What it hardcodes |
|---|---|---|
| `workspace_switch_gesture_update(delta_y, ...)` | delta axis | Takes `delta_y` — workspace switching uses Y |
| `WORKSPACE_GESTURE_MOVEMENT: f64 = 300.` | normalization | Against workspace height |

**Note**: Monitor-level workspace switching must remain on Y. This is not a bug — it is an
intentional design constraint. See Section 2.

---

### `src/layout/workspace.rs`

| Location | Symbol | What it hardcodes |
|---|---|---|
| 75 | `transform: Transform` | Stores output physical rotation; initialized from `output.current_transform()` (line 262); **not used for layout axis** — see Section 3 |
| 326 | `transform: Transform::Normal` | Default for no-output workspaces |
| 297–299 | `view_size`, `working_area` in `new_with_config_no_outputs` | Hardcoded `Size::from((1280., 720.))` — landscape assumption in test default |

---

### `src/layout/mod.rs`

| Location | Symbol | What it hardcodes |
|---|---|---|
| ~450 | `InteractiveMoveData { width: ColumnWidth, .. }` | Field named `width` — main-axis size concept is horizontal-named |
| 60 | imports | `use crate::layout::scrolling::ScrollDirection` — `Left`/`Right` bleeds into mod-level |

---

### `niri-config/src/animations.rs`

| Location | Symbol | What it hardcodes |
|---|---|---|
| 14 | `horizontal_view_movement: HorizontalViewMovementAnim` | **Public config key** that users write in `config.kdl`. Renaming is a breaking change. |
| 59 | `AnimationsPart::horizontal_view_movement` | Parsed from the config file directly |

This is the only user-visible name in the inventory. All others are internal Rust identifiers.

---

### `src/input/mod.rs`

| Location | Symbol | What it hardcodes |
|---|---|---|
| ~3835–3870 | 3-finger swipe axis decision | `if cx.abs() > cy.abs()` → `view_offset_gesture_begin` (horizontal); else → `workspace_switch_gesture_begin` (vertical) |
| ~3885 | gesture routing | `layout.workspace_switch_gesture_update(delta_y, ...)` — Y always goes to workspace switch |
| ~3910 | gesture routing | `layout.view_offset_gesture_update(delta_x, ...)` — X always goes to view offset |
| ~3304–3397 | overview scroll | vertical finger scroll → workspace switch; horizontal → view offset |
| wheel scroll | overview | vertical wheel → workspace switch; horizontal wheel → column focus |

---

### `src/input/scroll_swipe_gesture.rs`

| Location | Symbol | What it hardcodes |
|---|---|---|
| `update(dx, dy)` | axis detection | `self.vertical = dy != 0.` — determines which axis the gesture is on from the first non-zero delta |

---

### `src/layout/floating.rs`

Floating layout is inherently 2D (`pos: Point<f64, SizeFrac>` relative to working area). Both axes
are used symmetrically in `scale_by_working_area` (multiplies `pos.x * area.size.w` and
`pos.y * area.size.h`) and in `recompute_logical_pos` (clamps both axes). **No changes are needed
here** — floating layout is scroll-axis-agnostic.

---

### `src/layout/closing_window.rs`

Uses `Transform::Normal` when rendering to a texture (line 112–123). This is the physical output
texture rotation, not the layout axis. No axis assumptions about which direction windows close.
Closing window position is 2D `Point<f64, Logical>`.

---

### Tests (`src/layout/scrolling.rs:5568–5603`)

The only inline unit tests are:

- `working_area_starts_at_physical_pixel` (line 5576): tests that `compute_working_area` rounds
  to a physical pixel. Checks both `area.loc.x` and `area.loc.y` — axis-agnostic correctness.
- `large_fractional_strut` (line 5591): smoke test that `compute_working_area` doesn't panic with
  huge strut values. Also axis-agnostic.

The broader integration tests (under `src/layout/tests/`) use `new_with_config_no_outputs`, which
hardcodes `view_size = (1280., 720.)`. Any test that asserts on absolute X/Y coordinates of tiles
or columns implicitly assumes horizontal scroll. These will need updating or parameterizing when
the `Vertical` impl is introduced.

**Test scope resolved**: `src/layout/tests.rs` is a single ~2100-line file (not a directory) that
declares two sub-modules:

- `tests/animations.rs`: uses `insta::assert_snapshot!` with absolute `x`, `y`, `w`, `h`
  coordinates printed by `format_tiles`. A handful of snapshots. These will need regenerating if
  Horizontal geometry accidentally changes in Phase 2, and will need new Vertical snapshots in
  Phase 5. `insta` will catch any unintentional drift before it ships.
- `tests/fullscreen.rs`: uses `check_ops` for structural invariant checks only — no absolute
  coordinate assertions.

The main proptest tests in `tests.rs` check structural invariants (no panics, column/tile
consistency) against random `Op` sequences. They do **not** assert absolute coordinates.
**Phase 3 is not significantly longer than the doc suggests.**

---

## 2. Asymmetries

Places where X and Y are NOT freely interchangeable in the real world.

### 2.1 Gesture protocol distinguishes axes at the kernel level

libinput / the Wayland protocol delivers scroll events with separate `delta_x` and `delta_y`
values. A 3-finger swipe produces two independent scalar fields — they are not the same value on
two axes. The compositor must explicitly decide which axis routes where. Currently:

- `delta_x` → `view_offset_gesture_update` (within-workspace scroll)  
- `delta_y` → `workspace_switch_gesture_update` (between workspaces)

In vertical scroll mode, this routing must invert: `delta_y` would drive within-workspace scroll.
But workspace switching also needs `delta_y`. **This creates a true protocol-level conflict**: both
within-workspace scroll and workspace switching want the same gesture axis. There is no symmetric
solution — one or the other must move to a different gesture modifier (4-finger? ctrl+scroll?).
This is a design decision that cannot be resolved by code alone. See Section 6.

### 2.2 Workspace switching is permanently vertical

The monitor-level stack of workspaces is placed on the Y axis regardless of within-workspace
scroll direction (monitor.rs `workspaces_render_geo`). This is correct: workspaces are stacked
visually top-to-bottom on screen. Making workspace switching horizontal would conflict with the
natural mental model of workspace stacks. This asymmetry must be accepted as a constraint, not
worked around.

### 2.3 Output transforms (physical rotation) are orthogonal

Smithay's `Transform` (Normal/90/180/270/Flipped) describes the **physical display rotation**. A
display rotated 90° maps logical-X → physical-Y. This is handled entirely at the rendering
boundary (DRM/KMS or renderer). The compositor always reasons in **logical coordinates**, so
physical output rotation is invisible to layout code. See also Section 3.

Consequence: a rotated display does not change which axis `scrolling.rs` uses for its column
positions. The layout axis and the physical output axis are independent.

### 2.4 `ColumnWidth` and `WindowHeight` are user-facing config concepts

In horizontal mode: `ColumnWidth` is the main-axis (scroll-axis) dimension of a column;
`WindowHeight` is the cross-axis dimension of a tile within that column. In vertical mode these
roles swap:

- What the user currently configures as `ColumnWidth::Proportion(0.5)` would logically become
  "half the working height" in vertical mode.
- What the user currently configures as `WindowHeight::Fixed(400)` would become the window's
  **width** (cross-axis size) in vertical mode.

These concepts live in `niri-config` and are surfaced in user config files. Renaming or aliasing
them is a user-visible breaking change independent of the Rust code rename.

### 2.5 Strut semantics flip by axis

Layer-shell panels define exclusive zones on specific edges (Left/Right/Top/Bottom). In horizontal
scroll mode:

- Left/right struts shrink `working_area.size.w` → directly reduces the scroll-axis budget
- Top/bottom struts shrink `working_area.size.h` → reduces the cross-axis budget

In vertical scroll mode, left/right struts would shrink the **cross-axis** budget (tile width),
and top/bottom struts would shrink the **main-axis** budget (affecting how many rows fit). The
`compute_working_area` function applies all four independently, which is already correct —
but the intuition changes, and any documentation or user mental model of "struts affect how much
space columns have" would need updating.

### 2.6 Scroll-wheel event routing

Scroll wheel events report a `vertical_scroll` and `horizontal_scroll` value. The current routing
in the overview (`input/mod.rs ~3304–3397`) maps vertical wheel to workspace switching and
horizontal wheel to column movement. This routing is tied to physical scroll-wheel semantics, not
to the within-workspace scroll axis. In vertical scroll mode this routing would need to remain
deliberate.

---

## 3. Smithay Leverage

**Finding**: Smithay's `Transform` machinery cannot be reused to implement vertical scroll mode.
The layout must be parameterised at the `ScrollingSpace` level, entirely in logical coordinates.

### Evidence

`workspace.rs:75`:
```rust
transform: Transform,
```
`workspace.rs:262`:
```rust
transform: output.current_transform(),
```

The `transform` field is populated from `output.current_transform()`. Its only use is in
`send_scale_transform` calls that propagate the physical output rotation to Wayland window
surfaces, so clients know how their buffers should be oriented in physical space.

`scrolling.rs` does not import `Transform` at all. Scroll layout geometry is computed purely in
logical coordinates (`smithay::utils::Logical` coordinate space), which is by design: Smithay's
logical space is rotation-agnostic. The `Transform` applies at the physical pixel boundary (inside
the renderer or DRM layer), after the layout has finished placing everything.

**Conclusion**: There is no Smithay facility that can be hooked to flip the layout axis. The
rotation machinery rotates the rendered framebuffer; it does not change which dimension
`column_xs()` accumulates into. Vertical scroll mode is a purely layout-level change in
`ScrollingSpace<W>`.

---

## 4. Runtime Enum Design

**Decision 1** (see "Decisions Locked") selects a runtime enum with inherent methods over a
trait with type parameters. The file `src/layout/axis.rs` was created in Phase 0.

```rust
/// The scrolling axis for a workspace's tiling layout.
///
/// `main` axis = the scroll direction (X for Horizontal, Y for Vertical).
/// `cross` axis = perpendicular (Y for Horizontal, X for Vertical).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    Horizontal,  // columns left→right, view scrolls on X — current niri behaviour
    Vertical,    // rows top→bottom, view scrolls on Y — each row is one window wide
}
```

### Inherent methods

Each method takes `self` and branches on the variant. The `Horizontal` arm produces behaviour
identical to the current hard-coded niri code. The `Vertical` arm implements vertical-native
geometry.

| Method | Signature | Replaces |
|---|---|---|
| `main_size` | `(self, Size<f64,L>) -> f64` | `.w` at scrolling.rs:4401–4412; gesture norm at :3076 |
| `cross_size` | `(self, Size<f64,L>) -> f64` | `.h` for `max_tile_height` at :4490 |
| `make_size` | `(self, main, cross) -> Size` | Column/tile size construction |
| `main_coord` | `(self, Point<f64,L>) -> f64` | `.x` in `column_xs()` at :2304–2323 |
| `cross_coord` | `(self, Point<f64,L>) -> f64` | `.y` in `tiles_origin()` at :5181 |
| `make_point` | `(self, main, cross) -> Point` | Two-component Point construction |
| `set_main` | `(self, &mut Point, f64)` | `offset.x += ...` in `render_offset()` at :4160 |
| `set_cross` | `(self, &mut Point, f64)` | `origin.y += ...` in `tiles_origin()` at :5181 |
| `view_offset_point` | `(self, f64) -> Point` | `Point::from((-view_pos, 0.))` at :2381, :2402 |
| `column_origin_point` | `(self, f64) -> Point` | `Point::from((col_x, 0.))` at :2384, :2405 |
| `scroll_gesture_delta` | `(self, dx, dy) -> f64` | `delta_x` param in `view_offset_gesture_update` at :3059 |
| `switch_gesture_delta` | `(self, dx, dy) -> f64` | `delta_y` routing in `input/mod.rs` at ~:3885 |

### Vertical-native column semantics (Decision 2)

In Vertical mode a "column" contains exactly one tile spanning the full working-area width
(cross-axis). `tile_offsets_iter` degenerates to a single tile — the within-column stacking loop
is a no-op past index 0. `tiles_origin` positions the sole tile at the working-area cross-axis
origin; main-axis gaps go between columns, not within them.

`ColumnWidth` config values are silently ignored in Vertical mode with a debug log. Each column
always receives `cross_size(working_area.size)`.

Multi-tile columns cannot be migrated from Horizontal to Vertical workspaces. Vertical workspaces
start empty. Live mode-switching is out of scope (Decision 7).

### What stays outside `ScrollAxis`

- **Strut application** (`compute_working_area`): applies all four struts independently already.
- **Floating layout**: entirely 2D, scroll-axis-agnostic; no changes needed.
- **`ClosingWindow`, `Tile` animations**: 2D, no changes needed.
- **`ColumnData.width`**: rename to `main_size` in Phase 2 (internal, non-breaking).

---

## 5. Migration Strategy

The approach below keeps the code compiling and tests passing at every phase. No phase silences or
ignores tests.

### Phase 0 — Define `ScrollAxis` enum (COMPLETE)

1. Created `src/layout/axis.rs` with the `ScrollAxis` enum and all inherent methods.
2. Added `pub mod axis;` to `src/layout/mod.rs`.
3. `cargo check` and `cargo test --lib layout::axis` both pass.

**Done in Hour 3–4.** All subsequent phases are future work.

---

### Phase 1 — Add the axis field to `ScrollingSpace` and `Column`

1. Add `pub(super) axis: ScrollAxis` field to `ScrollingSpace<W>`.
2. Add `axis: ScrollAxis` field to `Column<W>` (needed for `tiles_origin()` and
   `tile_offsets_iter()`).
3. Initialize both fields to `ScrollAxis::Horizontal` everywhere they are constructed.
4. `cargo check` must pass. No behavior changes.

**Early-failure point**: Adding the field forces a compile error at every construction site for
`ScrollingSpace` and `Column`. Any missed site is caught here. No type parameter changes needed.

**Expected scope**: `ScrollingSpace::new`, `Column::new`, and any test fixture construction.
`FloatingSpace`, `Tile`, `Monitor`, `Workspace`, `Layout` — none of these need changes yet.

**Critical — axis propagation**: When `Column::new` is updated to accept `axis: ScrollAxis`,
every call site must pass `self.axis` from the enclosing `ScrollingSpace`. Do **not** use
`Default::default()` or the literal `ScrollAxis::Horizontal` at any `Column` construction site
inside `ScrollingSpace`. A `ScrollingSpace` with `axis: Vertical` that silently constructs
`Column` with `axis: Horizontal` would produce wrong geometry at runtime with no compile error.

---

### Phase 2 — Migrate geometry one call-site at a time

Replace hardcoded axis accesses with `self.axis.method()` calls. Each sub-step compiles
independently. Tests pass throughout because `Horizontal` produces identical results.

1. **`column_xs()`** (scrolling.rs:2304–2316):  
   `x += data.width + gaps` → `main_pos += data.width + gaps`  
   Rename `ColumnData.width` → `ColumnData.main_size`.

2. **`tiles_with_render_positions()`** and `_mut()` (scrolling.rs:2381, 2384, 2402, 2405):  
   `Point::from((-self.view_pos(), 0.))` → `self.axis.view_offset_point(self.view_pos())`  
   `Point::from((col_x, 0.))` → `self.axis.column_origin_point(col_x)`

3. **`Column::tiles_origin()`** (scrolling.rs:5169–5190):  
   `origin.y += working_area.loc.y + gaps` →  
   `self.axis.set_cross(&mut origin, self.axis.cross_coord(working_area.loc.into()) + gaps)`

4. **`Column::tile_offsets_iter()`** (scrolling.rs:5194–5239):  
   X-centering: `pos.x += (tiles_width - data.size.w) / 2.` →  
   `self.axis.set_cross(&mut pos, self.axis.cross_coord(pos) + (tiles_cross - data_cross) / 2.)`  
   Y accumulation: `origin.y += data.size.h + gaps` →  
   `self.axis.set_cross(&mut origin, self.axis.cross_coord(origin) + data_cross + gaps)`  
   *In Vertical mode this loop body skips past the first tile (single-tile column invariant).*

5. **`resolve_column_width()`** (scrolling.rs:4401–4412):  
   `working_size.w` → `self.axis.main_size(working_size)`

6. **`update_tile_sizes_with_transaction()`** (scrolling.rs:4490):  
   `working_size.h - gaps * 2.` → `self.axis.cross_size(working_size) - gaps * 2.`

7. **`compute_new_view_offset()` free fn** (scrolling.rs:5457–5489):  
   Rename parameters `cur_x, view_width, new_col_x, new_col_width` →  
   `cur_main, view_main, new_col_main, new_col_main_size`. Body is pure scalar arithmetic; no
   `self.axis` calls needed inside.

8. **`Column::render_offset()` and `animate_move_from()`** (scrolling.rs:4156–4188):  
   `offset.x += move_.from * move_.anim.value()` →  
   `self.axis.set_main(&mut offset, self.axis.main_coord(offset) + move_.from * move_.anim.value())`  
   Rename `from_x_offset` → `from_main_offset`.

---

### Phase 3 — Migrate gesture routing

1. **`view_offset_gesture_update(delta_x: f64, ...)`** (scrolling.rs:3059):  
   Rename parameter to `delta_main`. The caller passes
   `self.axis.scroll_gesture_delta(dx, dy)`.

2. **Gesture routing in `input/mod.rs`** (~3835–3917):  
   The current `if cx.abs() > cy.abs()` threshold routes horizontally. Replace with a query of
   the active workspace's `axis` field:
   - `Horizontal`: X swipe → view offset, Y swipe → workspace switch (current behavior unchanged)
   - `Vertical`: Y swipe → view offset, X swipe → workspace switch (Decision 3)

3. **`VIEW_GESTURE_WORKING_AREA_MOVEMENT` normalization** (scrolling.rs:3076):  
   `self.working_area.size.w` → `self.axis.main_size(self.working_area.size)`

**Where tests may fail**: Any test simulating a gesture and checking the resulting view offset.
Keep `axis: ScrollAxis::Horizontal` in all test workspace construction to avoid regression.

---

### Phase 4 — Animation config key (no change needed)

`animate_view_offset()` (scrolling.rs:684) references `options.animations.horizontal_view_movement.0`.

Per Decision 5: keep the name unchanged. Add a doc comment on `HorizontalViewMovementAnim`
explaining it applies to whichever axis is the scroll axis.

---

### Phase 5 — Wire Vertical mode to config and keybinds

1. Add `scroll_direction: ScrollAxis` to `niri-config/src/layout.rs` (default `Horizontal`).
   Config key: `scroll-direction "vertical"` inside a `layout { }` block.

2. In `Workspace::new_with_config`, read `options.layout.scroll_direction` and pass it when
   constructing `ScrollingSpace::new` and `Column::new`. No enum wrapper needed — `ScrollAxis`
   is already a plain field (from Phase 1).

3. In Vertical mode, `Column` enforces single-tile invariant: attempting to add a second tile is
   rejected with a debug log.

4. Add `focus-column-up`, `focus-column-down`, `move-column-up`, `move-column-down` keybind
   actions as aliases for `focus-column-right`, `focus-column-left`, `move-column-right`,
   `move-column-left` respectively (Decision 6). Both families work in both modes.

**This is the first phase with user-visible behavior change.**

---

### Cheapest early-failure points

- **Phase 1** catches all construction sites for `ScrollingSpace` and `Column` (missing-field
  error).
- **Phase 2 step 1** (`column_xs`) immediately verifies the `ColumnData` shape is right.
- **Phase 3** is the first phase that can break gesture integration tests; `axis: Horizontal` in
  test fixtures prevents regression.
- **Phase 5** is the first phase that can break gesture routing tests. Run full suite before and
  after.

---

## 6. Decisions Resolved

All questions from the original open section are now resolved.

### Q1. Per-workspace or global? → **Global** (Decision 4)

Config key `layout { scroll-direction "vertical"; }` applies to the entire compositor session.
Per-workspace was rejected because it would require the input-routing code to query the axis on
every gesture event and would create a confusing UX when moving windows between workspaces with
different axes.

### Q2. Gesture conflict in vertical mode → **Symmetric rotation** (Decision 3)

In Vertical mode: 3-finger Y swipe → within-workspace scroll; 3-finger X swipe → workspace
switching. This is the symmetric rotation of horizontal-mode behaviour. It works because
Vertical-native has no cross-axis stacking, so the X gesture axis is free for workspace switching.

### Q3. `horizontal_view_movement` config key → **Keep unchanged** (Decision 5)

The key name stays `horizontal_view_movement`. It applies to whichever axis is the scroll axis.
A doc comment on `HorizontalViewMovementAnim` will explain this. No breaking config change.

### Q4. `ColumnWidth` / `WindowHeight` concept names → **Keep unchanged, ignore in Vertical**

Config names are unchanged. In Vertical mode, `ColumnWidth` values are silently ignored (debug
log) because each column spans the full cross-axis. Users switching to Vertical mode do not need
to change their config.

### Q5. Column semantics in vertical mode → **Single-tile columns** (Decision 2)

A "column" in Vertical mode contains exactly one tile spanning the full working-area width
(cross-axis). No within-column stacking. `tile_offsets_iter` degenerates cleanly — the
within-column loop is a no-op past index 0. Attempting to add a second tile to a Vertical column
is rejected with a debug log.

### Q6. Keybind action names → **Both families, both modes** (Decision 6)

`focus-column-left/right` and the new `focus-column-up/down` both work in both modes. Switching
axis requires no keybind changes. `left` and `right` map to the scroll direction in both modes.

### Q7. IPC contract → **Out of scope** (Decision 7)

`WindowLayout.tile_pos_in_workspace_view` is already a 2D `(f64, f64)` field. No wire-format
changes. IPC consumers that assume horizontal scroll-axis semantics would need independent updates
outside this fork.

### Q8. Trait generics vs runtime enum → **Runtime enum** (Decision 1)

Phase 5 of the trait-based plan already required a `WorkspaceScrolling<W>` enum wrapper to store
both `<W, Horizontal>` and `<W, Vertical>` in the same `Workspace` field. Once that enum is
inevitable, static dispatch's advantage over a runtime `match` disappears — while the
type-parameter plague across `Column`, `ScrollingSpace`, `Workspace`, `Monitor`, and `Layout`
remains. The runtime enum eliminates both problems at negligible branch cost.
