use approx::assert_abs_diff_eq;
use niri_config::animations::{Curve, EasingParams, FocusFlashAnim, Kind};

use super::*;

const LINEAR_1S: Kind = Kind::Easing(EasingParams {
    duration_ms: 1000,
    curve: Curve::Linear,
});

/// Total flash duration 2000ms → 1000ms per phase after split (keeps advance timings simple).
const FOCUS_FLASH_LINEAR_2S: Kind = Kind::Easing(EasingParams {
    duration_ms: 2000,
    curve: Curve::Linear,
});

fn focus_flash_anim(target: f64) -> FocusFlashAnim {
    FocusFlashAnim {
        anim: niri_config::Animation {
            off: false,
            kind: FOCUS_FLASH_LINEAR_2S,
        },
        min_opacity: target,
    }
}

fn make_options(target: f64) -> Options {
    let mut options = Options {
        layout: niri_config::Layout {
            gaps: 0.0,
            ..Default::default()
        },
        ..Options::default()
    };
    options.animations.focus_flash = focus_flash_anim(target);
    options.animations.window_movement.0.kind = LINEAR_1S;
    options
}

fn set_up(target: f64) -> Layout<TestWindow> {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::CompleteAnimations,
    ];
    check_ops_with_options(make_options(target), ops)
}

fn tile(layout: &Layout<TestWindow>, id: usize) -> &Tile<TestWindow> {
    if let Some(InteractiveMoveState::Moving(move_)) = &layout.interactive_move {
        if *move_.tile.window().id() == id {
            return &move_.tile;
        }
    }

    layout
        .active_workspace()
        .unwrap()
        .tiles_with_render_positions()
        .map(|(tile, _, _)| tile)
        .find(|tile| *tile.window().id() == id)
        .expect("tile not found")
}

fn tile_mut(layout: &mut Layout<TestWindow>, id: usize) -> &mut Tile<TestWindow> {
    let in_move = matches!(
        &layout.interactive_move,
        Some(InteractiveMoveState::Moving(move_)) if *move_.tile.window().id() == id
    );
    if in_move {
        let Some(InteractiveMoveState::Moving(move_)) = &mut layout.interactive_move else {
            unreachable!()
        };
        return &mut move_.tile;
    }

    layout
        .active_workspace_mut()
        .unwrap()
        .tiles_mut()
        .find(|tile| *tile.window().id() == id)
        .expect("tile not found")
}

fn flash_value(layout: &Layout<TestWindow>, id: usize) -> Option<f64> {
    tile(layout, id)
        .alpha_animation
        .as_ref()
        .filter(|alpha| alpha.is_focus_flash())
        .map(|alpha| alpha.anim.clamped_value())
}

fn flash_to(layout: &Layout<TestWindow>, id: usize) -> Option<f64> {
    tile(layout, id)
        .alpha_animation
        .as_ref()
        .filter(|alpha| alpha.is_focus_flash())
        .map(|alpha| alpha.anim.to())
}

fn flash_has_then(layout: &Layout<TestWindow>, id: usize) -> Option<bool> {
    tile(layout, id)
        .alpha_animation
        .as_ref()
        .filter(|alpha| alpha.is_focus_flash())
        .map(|alpha| alpha.focus_flash_then().is_some())
}

fn has_flash(layout: &Layout<TestWindow>, id: usize) -> bool {
    tile(layout, id)
        .alpha_animation
        .as_ref()
        .is_some_and(|alpha| alpha.is_focus_flash())
}

fn alpha_value(layout: &Layout<TestWindow>, id: usize) -> Option<f64> {
    tile(layout, id)
        .alpha_animation
        .as_ref()
        .map(|alpha| alpha.anim.clamped_value())
}

fn alpha_to(layout: &Layout<TestWindow>, id: usize) -> Option<f64> {
    tile(layout, id)
        .alpha_animation
        .as_ref()
        .map(|alpha| alpha.anim.to())
}

#[test]
fn flash_two_phase_round_trip() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    layout.verify_invariants();

    assert_eq!(flash_to(&layout, 1), Some(0.5));
    assert_eq!(flash_has_then(&layout, 1), Some(true));
    assert_abs_diff_eq!(flash_value(&layout, 1).unwrap(), 1.0, epsilon = 1e-6);
    assert!(tile(&layout, 1).are_animations_ongoing());
    assert!(!has_flash(&layout, 2));

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);
    layout.verify_invariants();
    assert_abs_diff_eq!(flash_value(&layout, 1).unwrap(), 0.75, epsilon = 1e-6);

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);
    layout.verify_invariants();
    // First phase done → restore starts at target.
    assert_eq!(flash_to(&layout, 1), Some(1.0));
    assert_eq!(flash_has_then(&layout, 1), Some(false));
    assert_abs_diff_eq!(flash_value(&layout, 1).unwrap(), 0.5, epsilon = 1e-6);
    assert!(tile(&layout, 1).are_animations_ongoing());

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);
    layout.verify_invariants();
    assert_abs_diff_eq!(flash_value(&layout, 1).unwrap(), 0.75, epsilon = 1e-6);

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(!tile(&layout, 1).are_animations_ongoing());
}

#[test]
fn off_config_is_noop() {
    let mut options = make_options(0.5);
    options.animations.focus_flash.anim.off = true;
    let mut layout = check_ops_with_options(
        options,
        [
            Op::AddOutput(1),
            Op::AddWindow {
                params: TestWindowParams::new(1),
            },
            Op::CompleteAnimations,
        ],
    );

    layout.animate_focus_flash(&1);
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
}

#[test]
fn target_near_one_is_noop() {
    let mut layout = set_up(1.0);
    layout.animate_focus_flash(&1);
    assert!(!has_flash(&layout, 1));

    tile_mut(&mut layout, 1).animate_focus_flash(
        1.0 + 1e-9,
        niri_config::Animation {
            off: false,
            kind: FOCUS_FLASH_LINEAR_2S,
        },
    );
    assert!(!has_flash(&layout, 1));
}

#[test]
fn target_is_clamped_to_unit_interval() {
    let mut layout = set_up(0.5);
    let config = niri_config::Animation {
        off: false,
        kind: FOCUS_FLASH_LINEAR_2S,
    };

    tile_mut(&mut layout, 1).animate_focus_flash(-0.25, config);
    layout.focus_flash_in_flight = true;
    layout.verify_invariants();
    assert_eq!(flash_to(&layout, 1), Some(0.0));
    assert_eq!(flash_has_then(&layout, 1), Some(true));
}

#[test]
fn skips_when_alpha_already_animating() {
    let mut layout = set_up(0.5);
    // Fade-in toward opaque is a valid visible-tile alpha animation.
    tile_mut(&mut layout, 1).animate_alpha(
        0.0,
        1.0,
        niri_config::Animation {
            off: false,
            kind: LINEAR_1S,
        },
    );
    layout.verify_invariants();

    layout.animate_focus_flash(&1);
    layout.verify_invariants();

    assert!(!has_flash(&layout, 1));
    assert_eq!(alpha_to(&layout, 1), Some(1.0));
    assert_abs_diff_eq!(alpha_value(&layout, 1).unwrap(), 0.0, epsilon = 1e-6);
}

#[test]
fn skips_during_open_animation() {
    let mut layout = set_up(0.5);
    tile_mut(&mut layout, 1).start_open_animation();
    assert!(tile(&layout, 1).are_animations_ongoing());

    layout.animate_focus_flash(&1);
    assert!(!has_flash(&layout, 1));
}

#[test]
fn skips_when_clock_completes_instantly() {
    let mut layout = set_up(0.5);
    layout.clock.set_complete_instantly(true);
    layout.animate_focus_flash(&1);
    assert!(!has_flash(&layout, 1));
}

#[test]
fn repeated_flash_while_active_restarts_from_current() {
    let mut layout = set_up(0.4);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 250 }.apply(&mut layout);
    let mid = flash_value(&layout, 1).unwrap();

    layout.animate_focus_flash(&1);
    layout.verify_invariants();

    assert_abs_diff_eq!(flash_value(&layout, 1).unwrap(), mid, epsilon = 1e-6);
    assert_eq!(flash_to(&layout, 1), Some(0.4));
    assert_eq!(flash_has_then(&layout, 1), Some(true));
}

#[test]
fn ensure_alpha_to_1_hard_clears_outbound_phase() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);
    let mid = flash_value(&layout, 1).unwrap();
    assert!(mid < 1.0);

    tile_mut(&mut layout, 1).ensure_alpha_animates_to_1();
    layout.advance_animations();
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(tile(&layout, 1).alpha_animation.is_none());
    assert!(!tile(&layout, 1).are_animations_ongoing());
    assert!(!layout.focus_flash_in_flight);
}

#[test]
fn ensure_alpha_to_1_hard_clears_return_phase() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 1000 }.apply(&mut layout);
    assert_eq!(flash_to(&layout, 1), Some(1.0));
    assert_eq!(flash_has_then(&layout, 1), Some(false));

    tile_mut(&mut layout, 1).ensure_alpha_animates_to_1();
    layout.advance_animations();
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(tile(&layout, 1).alpha_animation.is_none());
    assert!(!layout.focus_flash_in_flight);
}

#[test]
fn animate_alpha_mid_flash_takes_over() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);

    tile_mut(&mut layout, 1).animate_alpha(
        0.0,
        1.0,
        niri_config::Animation {
            off: false,
            kind: LINEAR_1S,
        },
    );
    layout.advance_animations();
    layout.verify_invariants();

    assert!(!has_flash(&layout, 1));
    assert!(!layout.focus_flash_in_flight);
    assert_eq!(alpha_to(&layout, 1), Some(1.0));
    // Continues from the mid-flash value rather than jumping.
    assert_abs_diff_eq!(alpha_value(&layout, 1).unwrap(), 0.75, epsilon = 1e-6);
}

#[test]
fn ongoing_across_phase_boundary_before_advance() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);

    // Drive the clock to the end of phase 1 without advancing (so restore is still pending).
    let mut now = layout.clock.now_unadjusted();
    now = now.saturating_add(Duration::from_millis(1000));
    layout.clock.set_unadjusted(now);

    let flash = tile(&layout, 1).alpha_animation.as_ref().unwrap();
    assert!(flash.is_focus_flash());
    assert!(flash.anim.is_done());
    assert!(flash.focus_flash_then().is_some());
    assert!(tile(&layout, 1).are_transitions_ongoing());

    layout.advance_animations();
    layout.verify_invariants();
    assert_eq!(flash_to(&layout, 1), Some(1.0));
    assert_eq!(flash_has_then(&layout, 1), Some(false));
}

#[test]
fn interactive_move_alpha_blocks_flash() {
    let mut layout = set_up(0.5);
    check_ops_on_layout(
        &mut layout,
        [
            Op::InteractiveMoveBegin {
                window: 1,
                output_idx: 1,
                px: 0.,
                py: 0.,
            },
            // Cross the rubberband threshold so the tile enters Moving with held alpha.
            Op::InteractiveMoveUpdate {
                window: 1,
                dx: 300.,
                dy: 0.,
                output_idx: 1,
                px: 300.,
                py: 0.,
            },
        ],
    );

    assert_eq!(alpha_to(&layout, 1), Some(INTERACTIVE_MOVE_ALPHA));
    assert!(tile(&layout, 1)
        .alpha_animation
        .as_ref()
        .unwrap()
        .hold_after_done());

    layout.animate_focus_flash(&1);
    layout.verify_invariants();

    assert!(!has_flash(&layout, 1));
    assert_eq!(alpha_to(&layout, 1), Some(INTERACTIVE_MOVE_ALPHA));
    assert!(tile(&layout, 1)
        .alpha_animation
        .as_ref()
        .unwrap()
        .hold_after_done());
}

#[test]
fn focus_switch_hard_clears_previous_flash() {
    let mut layout = set_up(0.6);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 250 }.apply(&mut layout);
    assert!(has_flash(&layout, 1));

    layout.animate_focus_flash(&2);
    layout.verify_invariants();

    // Previous tile snaps opaque — no concurrent offscreen restore.
    assert!(!has_flash(&layout, 1));
    assert!(tile(&layout, 1).alpha_animation.is_none());

    assert_eq!(flash_to(&layout, 2), Some(0.6));
    assert_eq!(flash_has_then(&layout, 2), Some(true));
}

#[test]
fn focus_return_after_leave_flashes_again() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 250 }.apply(&mut layout);

    layout.animate_focus_flash(&2);
    assert!(!has_flash(&layout, 1));

    // Returning to 1 starts a full flash from opaque.
    layout.animate_focus_flash(&1);
    layout.verify_invariants();
    assert_eq!(flash_to(&layout, 1), Some(0.5));
    assert_eq!(flash_has_then(&layout, 1), Some(true));
    assert_abs_diff_eq!(flash_value(&layout, 1).unwrap(), 1.0, epsilon = 1e-6);
    assert!(!has_flash(&layout, 2));
}

#[test]
fn missing_window_clears_in_flight() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    assert!(has_flash(&layout, 1));

    // Unknown id: cannot start a flash; still hard-clear any in-flight flash.
    layout.animate_focus_flash(&99);
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(!has_flash(&layout, 2));
    assert!(tile(&layout, 1).alpha_animation.is_none());
}

#[test]
fn complete_animations_finishes_both_phases() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::CompleteAnimations.apply(&mut layout);
    layout.verify_invariants();

    assert!(!has_flash(&layout, 1));
    assert!(!tile(&layout, 1).are_animations_ongoing());
}

#[test]
fn interactive_move_detach_replaces_active_flash() {
    let mut layout = set_up(0.5);
    check_ops_on_layout(
        &mut layout,
        [Op::InteractiveMoveBegin {
            window: 1,
            output_idx: 1,
            px: 0.,
            py: 0.,
        }],
    );

    // Still in Starting rubberband: tile remains in the layout, so flash can begin.
    layout.animate_focus_flash(&1);
    layout.verify_invariants();
    assert_eq!(flash_has_then(&layout, 1), Some(true));

    // Detach into Moving replaces opacity with held interactive-move alpha.
    // Apply without check_ops so we can sync the in-flight flag before verify.
    Op::InteractiveMoveUpdate {
        window: 1,
        dx: 300.,
        dy: 0.,
        output_idx: 1,
        px: 300.,
        py: 0.,
    }
    .apply(&mut layout);
    layout.advance_animations();
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(!layout.focus_flash_in_flight);
    assert_eq!(alpha_to(&layout, 1), Some(INTERACTIVE_MOVE_ALPHA));
    assert!(tile(&layout, 1)
        .alpha_animation
        .as_ref()
        .unwrap()
        .hold_after_done());
}

#[test]
fn hold_after_done_alpha_still_blocks_flash() {
    let mut layout = set_up(0.5);
    tile_mut(&mut layout, 1).animate_alpha(
        1.0,
        0.3,
        niri_config::Animation {
            off: false,
            kind: LINEAR_1S,
        },
    );
    tile_mut(&mut layout, 1).hold_alpha_animation_after_done();
    Op::AdvanceAnimations { msec_delta: 1000 }.apply(&mut layout);

    // Held alpha remains present after completion.
    assert!(tile(&layout, 1)
        .alpha_animation
        .as_ref()
        .unwrap()
        .anim
        .is_done());
    assert!(tile(&layout, 1)
        .alpha_animation
        .as_ref()
        .unwrap()
        .hold_after_done());

    layout.animate_focus_flash(&1);
    assert!(!has_flash(&layout, 1));
    assert_eq!(alpha_to(&layout, 1), Some(0.3));
}

#[test]
fn clear_focus_flashes_except_hard_clears_others() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 400 }.apply(&mut layout);
    let mid = flash_value(&layout, 1).unwrap();
    assert!(has_flash(&layout, 1));
    assert!(mid < 1.0);

    layout.clear_focus_flashes_except(Some(&2));
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(tile(&layout, 1).alpha_animation.is_none());
}

#[test]
fn clear_when_off_still_clears_injected_flash() {
    // anim.off must not skip hard-clear: config reload mid-flash should still drop offscreen.
    let mut options = make_options(0.5);
    options.animations.focus_flash.anim.off = true;
    let mut layout = check_ops_with_options(
        options,
        [
            Op::AddOutput(1),
            Op::AddWindow {
                params: TestWindowParams::new(1),
            },
            Op::CompleteAnimations,
        ],
    );

    let config = niri_config::Animation {
        off: false,
        kind: FOCUS_FLASH_LINEAR_2S,
    };
    tile_mut(&mut layout, 1).animate_focus_flash(0.5, config);
    layout.focus_flash_in_flight = true;
    assert!(has_flash(&layout, 1));

    layout.clear_focus_flashes_except(None);
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));

    tile_mut(&mut layout, 1).animate_focus_flash(0.5, config);
    layout.focus_flash_in_flight = true;
    assert!(has_flash(&layout, 1));
    layout.clear_focus_flashes();
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
}

#[test]
fn seat_path_animate_when_off_hard_clears_in_flight() {
    // Seat path calls animate_focus_flash (not clear_*) when focusing a window.
    // After config reload to off, that call must still clear the previous flash.
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 400 }.apply(&mut layout);
    assert!(has_flash(&layout, 1));

    let mut options = make_options(0.5);
    options.animations.focus_flash.anim.off = true;
    layout.update_options(options);

    layout.animate_focus_flash(&2);
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(!has_flash(&layout, 2));
    assert!(tile(&layout, 1).alpha_animation.is_none());
}

#[test]
fn clear_focus_flashes_hard_drops_offscreen() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 400 }.apply(&mut layout);
    assert!(has_flash(&layout, 1));
    assert!(tile(&layout, 1).are_animations_ongoing());

    layout.clear_focus_flashes();
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(!tile(&layout, 1).are_animations_ongoing());
    assert!(tile(&layout, 1).alpha_animation.is_none());
}

#[test]
fn clear_focus_flashes_is_noop_when_none_in_flight() {
    let mut layout = set_up(0.5);
    assert!(!layout.focus_flash_in_flight);
    layout.clear_focus_flashes();
    assert!(!layout.focus_flash_in_flight);
    layout.verify_invariants();
}

#[test]
fn floating_window_flashes() {
    let mut layout = set_up(0.5);
    check_ops_on_layout(&mut layout, [Op::ToggleWindowFloating { id: Some(1) }]);
    assert!(layout.active_workspace().unwrap().is_floating(&1));

    layout.animate_focus_flash(&1);
    layout.verify_invariants();
    assert_eq!(flash_to(&layout, 1), Some(0.5));
    assert_eq!(flash_has_then(&layout, 1), Some(true));
}

#[test]
fn window_rule_opacity_and_flash_alpha_compose_as_product() {
    // Documents the intended composition: win_alpha × tile_alpha (render multiplies them).
    let mut layout = check_ops_with_options(
        make_options(0.5),
        [
            Op::AddOutput(1),
            Op::AddWindow {
                params: TestWindowParams {
                    rules: Some(ResolvedWindowRules {
                        opacity: Some(0.5),
                        ..ResolvedWindowRules::default()
                    }),
                    ..TestWindowParams::new(1)
                },
            },
            Op::CompleteAnimations,
        ],
    );

    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 1000 }.apply(&mut layout);
    layout.verify_invariants();

    let win_alpha = tile(&layout, 1)
        .window()
        .rules()
        .opacity
        .unwrap_or(1.)
        .clamp(0., 1.);
    let tile_alpha = flash_value(&layout, 1).unwrap() as f32;
    assert_abs_diff_eq!(tile_alpha, 0.5, epsilon = 1e-5);
    assert_abs_diff_eq!(win_alpha * tile_alpha, 0.25, epsilon = 1e-5);

    // Default rules compose as 1 × tile_alpha.
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 1000 }.apply(&mut layout);
    let win_alpha = tile(&layout, 1)
        .window()
        .rules()
        .opacity
        .unwrap_or(1.)
        .clamp(0., 1.);
    let tile_alpha = flash_value(&layout, 1).unwrap() as f32;
    assert_abs_diff_eq!(win_alpha * tile_alpha, 0.5, epsilon = 1e-5);
}

#[test]
fn natural_completion_clears_in_flight_flag() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    assert!(layout.focus_flash_in_flight);
    assert!(has_flash(&layout, 1));

    // One advance only starts the restore; a second finishes it and syncs the flag.
    Op::AdvanceAnimations { msec_delta: 1000 }.apply(&mut layout);
    assert!(has_flash(&layout, 1));
    assert!(layout.focus_flash_in_flight);

    Op::AdvanceAnimations { msec_delta: 1000 }.apply(&mut layout);
    layout.verify_invariants();

    assert!(!has_flash(&layout, 1));
    assert!(!layout.focus_flash_in_flight);

    // Later clear must stay a cheap no-op after natural completion.
    layout.clear_focus_flashes();
    assert!(!layout.focus_flash_in_flight);
}

#[test]
fn update_options_to_off_hard_clears_without_focus_change() {
    let mut layout = set_up(0.5);
    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 400 }.apply(&mut layout);
    assert!(has_flash(&layout, 1));
    assert!(layout.focus_flash_in_flight);

    let mut options = make_options(0.5);
    options.animations.focus_flash.anim.off = true;
    layout.update_options(options);
    layout.verify_invariants();

    assert!(!has_flash(&layout, 1));
    assert!(!layout.focus_flash_in_flight);
    assert!(tile(&layout, 1).alpha_animation.is_none());
}

#[test]
fn spring_kind_is_noop_programmatically() {
    let mut layout = set_up(0.5);
    tile_mut(&mut layout, 1).animate_focus_flash(
        0.5,
        niri_config::Animation {
            off: false,
            kind: Kind::Spring(niri_config::animations::SpringParams {
                damping_ratio: 1.0,
                stiffness: 800,
                epsilon: 0.001,
            }),
        },
    );
    layout.verify_invariants();
    assert!(!has_flash(&layout, 1));
    assert!(!layout.focus_flash_in_flight);
}

#[test]
fn tabbed_column_tab_switch_then_focus_flash_clears_previous() {
    let mut layout = set_up(0.5);
    check_ops_on_layout(
        &mut layout,
        [
            Op::ConsumeWindowIntoColumn,
            Op::ToggleColumnTabbedDisplay,
            Op::CompleteAnimations,
        ],
    );

    layout.animate_focus_flash(&1);
    Op::AdvanceAnimations { msec_delta: 400 }.apply(&mut layout);
    assert!(has_flash(&layout, 1));

    // Inactive tab may keep its flash (not drawn). Seat-equivalent flash on the new tab
    // hard-clears the previous tile.
    check_ops_on_layout(&mut layout, [Op::FocusWindowDown]);
    layout.animate_focus_flash(&2);
    layout.verify_invariants();

    assert!(!has_flash(&layout, 1));
    assert!(has_flash(&layout, 2));
    assert!(layout.focus_flash_in_flight);
}

#[test]
fn workspace_focus_change_can_flash() {
    let mut layout = set_up(0.5);
    check_ops_on_layout(
        &mut layout,
        [
            Op::FocusWindow(2),
            Op::MoveWindowToWorkspace {
                window_id: Some(2),
                workspace_idx: 1,
            },
            Op::FocusWorkspace(1),
            Op::CompleteAnimations,
        ],
    );

    // Focusing a window after a workspace switch (layout-level) can start a flash.
    layout.animate_focus_flash(&2);
    layout.verify_invariants();
    assert!(has_flash(&layout, 2));
    assert!(layout.focus_flash_in_flight);
}

#[test]
fn rapid_focus_while_tab_fade_alpha_blocks_flash() {
    let mut layout = set_up(0.5);
    check_ops_on_layout(
        &mut layout,
        [
            Op::ConsumeWindowIntoColumn,
            Op::ToggleColumnTabbedDisplay,
            Op::CompleteAnimations,
        ],
    );

    // Simulate an in-progress tab fade owning the alpha channel on window 2.
    tile_mut(&mut layout, 2).animate_alpha(
        0.0,
        1.0,
        niri_config::Animation {
            off: false,
            kind: LINEAR_1S,
        },
    );
    assert!(!has_flash(&layout, 2));
    assert_eq!(alpha_to(&layout, 2), Some(1.0));

    layout.animate_focus_flash(&2);
    layout.verify_invariants();

    assert!(!has_flash(&layout, 2));
    assert_eq!(alpha_to(&layout, 2), Some(1.0));
    assert!(!layout.focus_flash_in_flight);
}
