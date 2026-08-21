use super::*;

#[test]
fn window_state_timer_is_only_needed_for_floating_surfaces() {
    let mut theme = ThemeDocument::starter();
    assert!(!theme_has_floating_surface(&theme));

    theme.surfaces[0].placement.nest = SurfaceNest::Floating;
    assert!(theme_has_floating_surface(&theme));

    theme.surfaces[0].placement.nest = SurfaceNest::Auto;
    theme.surfaces[0].placement.reference.region = ReferenceRegion::Monitor;
    assert!(theme_has_floating_surface(&theme));
}

#[test]
fn center_points_keep_a_surface_centered_as_it_resizes() {
    assert_eq!(aligned_origin(0, 1920, 300, 0.5, 0.5, 0), 810);
    assert_eq!(aligned_origin(0, 1920, 500, 0.5, 0.5, 0), 710);
}

#[test]
fn reference_left_to_surface_right_places_widget_before_reference() {
    assert_eq!(aligned_origin(1600, 320, 300, 0.0, 1.0, 0), 1300);
}

#[test]
fn negative_offsets_inset_right_bottom_anchored_desktop_surfaces() {
    assert_eq!(aligned_origin(0, 3440, 198, 1.0, 1.0, -26), 3216);
    assert_eq!(aligned_origin(0, 1440, 144, 1.0, 1.0, -54), 1242);
}

#[test]
fn theme_dimensions_scale_from_logical_to_physical_pixels() {
    assert_eq!(scaled_theme_dimension(217, 1.0), 217);
    assert_eq!(scaled_theme_dimension(217, 1.25), 271);
    assert_eq!(scaled_theme_dimension(217, 1.5), 326);
    assert_eq!(scaled_theme_dimension(46, 2.0), 92);
}

#[test]
fn physical_host_dimensions_are_normalized_to_logical_pixels() {
    assert_eq!(logical_host_dimension(38, 1.25), 30);
    assert_eq!(logical_host_dimension(46, 1.0), 46);
    assert_eq!(logical_host_dimension(92, 2.0), 46);
    assert_eq!(logical_host_dimension(30, 0.0), 30);
}

#[test]
fn legacy_physical_offset_becomes_a_leftward_logical_theme_offset() {
    assert_eq!(legacy_offset_to_theme_offset(120, 1.25), -96);
    assert_eq!(legacy_offset_to_theme_offset(120, 1.0), -120);
    assert_eq!(legacy_offset_to_theme_offset(-5, 1.0), 0);
    assert_eq!(legacy_offset_to_theme_offset(20, 0.0), -20);
}

#[test]
fn tray_widget_action_targets_a_custom_theme_root_without_a_main_id() {
    let mut theme = ThemeDocument::starter();
    theme.surfaces[0].id = "layer-62744-2".into();
    let (surface_index, root_id) = context_menu_widget_origin(&theme).unwrap();
    assert_eq!((surface_index, root_id.as_str()), (0, "layer-62744-2"));

    let runtime = ThemeRuntime::new(true, true, true);
    let mut overrides = HashMap::new();
    theme_engine::execute_mouse_actions(
        &theme,
        surface_index,
        &root_id,
        "toggle(self, render)",
        None,
        runtime,
        &mut overrides,
    )
    .unwrap();
    let hidden = theme_engine::apply_mouse_action_overrides(&theme, &overrides);
    assert!(!theme_engine::surface_should_render(
        &hidden,
        surface_index,
        None,
        runtime
    ));
}

#[test]
fn fullscreen_bounds_cover_the_monitor_but_maximized_work_area_does_not() {
    let monitor = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    assert!(rect_covers_monitor(monitor, monitor));
    assert!(rect_covers_monitor(
        RECT {
            left: -2,
            top: -2,
            right: 1922,
            bottom: 1082,
        },
        monitor,
    ));
    assert!(!rect_covers_monitor(
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        },
        monitor,
    ));
}

#[test]
fn a_display_without_a_notification_area_keeps_the_widget_on_screen() {
    // A secondary monitor spanning -1920..0, with a taskbar across it.
    const TASKBAR_LEFT: i32 = -1920;
    const TASKBAR_WIDTH: i32 = 1920;
    const WIDGET: i32 = 118;

    // Beside a notification area the widget's right edge meets the anchor.
    // Measured from a taskbar's left edge instead — which is all a display
    // without a tray can offer — that same pairing lands a full width past the
    // screen edge, which is how the widget went missing.
    let (_, beside_tray) = placement_for_display(true);
    let off_screen = aligned_origin(
        TASKBAR_LEFT,
        TASKBAR_WIDTH,
        WIDGET,
        0.0,
        horizontal_anchor_factor(beside_tray),
        0,
    );
    assert_eq!(off_screen, TASKBAR_LEFT - WIDGET);

    // The anchor picked for such a display keeps it within the monitor.
    let (region, anchor) = placement_for_display(false);
    assert_eq!(region, ReferenceRegion::Taskbar);
    let on_screen = aligned_origin(
        TASKBAR_LEFT,
        TASKBAR_WIDTH,
        WIDGET,
        0.0,
        horizontal_anchor_factor(anchor),
        0,
    );
    assert_eq!(on_screen, TASKBAR_LEFT);
    assert!(on_screen >= TASKBAR_LEFT);
    assert!(on_screen + WIDGET <= TASKBAR_LEFT + TASKBAR_WIDTH);
}
