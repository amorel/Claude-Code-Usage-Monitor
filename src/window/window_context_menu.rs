use super::*;

pub(super) fn show_context_menu_document(
    hwnd: HWND,
    reference: Option<&str>,
    origin: Option<(usize, String)>,
) {
    let document = match context_menu::resolve_context_menu(reference) {
        Ok(document) => document,
        Err(error) => {
            diagnose::log(format!("context menu load failed: {error}"));
            context_menu::classic_context_menu()
        }
    };
    let language = lock_state()
        .as_ref()
        .map(|state| state.language)
        .unwrap_or_else(localization::detect_system_language);
    let data_context = context_menu_data_context(origin.as_ref());
    let mut actions = Vec::new();
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        append_context_menu_items(
            menu,
            &document.items,
            language,
            &data_context,
            origin.as_ref(),
            &mut actions,
        );
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            point.x,
            point.y,
            0,
            hwnd,
            None,
        )
        .0 as usize;
        let _ = DestroyMenu(menu);
        if selected >= 1_000 {
            if let Some(action) = actions.get(selected - 1_000).cloned() {
                execute_context_menu_action(hwnd, action, origin);
            }
        }
    }
}

pub(super) fn context_menu_data_context(origin: Option<&(usize, String)>) -> DataContext {
    let state = lock_state();
    let Some(state) = state.as_ref() else {
        return DataContext::from_usage(None, &Canvas::default());
    };
    let mut runtime = theme_runtime_from_state(state);
    let mut canvas = Canvas::default();
    if let Some(theme) = effective_theme_from_state(state) {
        let surface_index = origin.map_or(0, |(surface_index, _)| *surface_index);
        if theme.surfaces.get(surface_index).is_some() {
            runtime = theme_runtime_for_surface(&theme, surface_index, runtime);
            let (width, height) = theme_engine::resolve_surface_size(
                &theme,
                surface_index,
                state.data.as_ref(),
                runtime,
            );
            canvas.width = width;
            canvas.height = height;
        }
    }
    DataContext::from_usage_with_runtime(state.data.as_ref(), &canvas, runtime)
}

unsafe fn append_context_menu_items(
    menu: HMENU,
    items: &[ContextMenuItem],
    language: LanguageId,
    context: &DataContext,
    origin: Option<&(usize, String)>,
    actions: &mut Vec<ContextMenuAction>,
) {
    for item in items {
        match &item.kind {
            ContextMenuItemKind::Separator => {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            }
            ContextMenuItemKind::Text => {
                let label = native_interop::wide_str(&context_menu::rendered_label(
                    language,
                    &item.label,
                    context,
                ));
                let _ = AppendMenuW(menu, MF_GRAYED, 0, PCWSTR::from_raw(label.as_ptr()));
            }
            ContextMenuItemKind::Submenu { items } => {
                let Ok(submenu) = CreatePopupMenu() else {
                    continue;
                };
                append_context_menu_items(submenu, items, language, context, origin, actions);
                let label = native_interop::wide_str(&context_menu::rendered_label(
                    language,
                    &item.label,
                    context,
                ));
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP,
                    submenu.0 as usize,
                    PCWSTR::from_raw(label.as_ptr()),
                );
            }
            ContextMenuItemKind::Action { action } => {
                let id = 1_000 + actions.len();
                let label = native_interop::wide_str(&context_menu::rendered_label(
                    language,
                    &item.label,
                    context,
                ));
                let flags = context_menu_action_flags(action, origin);
                let _ = AppendMenuW(menu, flags, id, PCWSTR::from_raw(label.as_ptr()));
                actions.push(action.clone());
            }
        }
    }
}

pub(super) fn context_menu_action_flags(
    action: &ContextMenuAction,
    origin: Option<&(usize, String)>,
) -> MENU_ITEM_FLAGS {
    let state = lock_state();
    let Some(state) = state.as_ref() else {
        return MENU_ITEM_FLAGS(0);
    };
    let checked = match action {
        ContextMenuAction::SetUpdateFrequency { seconds } => {
            state.poll_interval_ms == seconds.saturating_mul(1_000)
        }
        ContextMenuAction::ToggleProvider { provider } => state.providers.contains(*provider),
        ContextMenuAction::ToggleStartup => is_startup_enabled(),
        ContextMenuAction::ToggleWidget => state
            .active_theme
            .as_ref()
            .and_then(|theme| {
                let effective = theme_engine::apply_mouse_action_overrides(
                    theme,
                    &state.mouse_action_overrides,
                );
                context_menu_widget_origin(&effective).map(|(surface_index, _)| {
                    let runtime = theme_runtime_for_surface(
                        &effective,
                        surface_index,
                        theme_runtime_from_state(state),
                    );
                    theme_engine::surface_should_render(
                        &effective,
                        surface_index,
                        state.data.as_ref(),
                        runtime,
                    )
                })
            })
            .unwrap_or(false),
        ContextMenuAction::SetLanguage { language } => {
            if language.eq_ignore_ascii_case("system") {
                state.language_override.is_none()
            } else {
                state
                    .language_override
                    .is_some_and(|current| current.code().eq_ignore_ascii_case(language))
            }
        }
        ContextMenuAction::SetDisplay { display } => state
            .active_theme
            .as_ref()
            .and_then(|theme| theme.surfaces.first())
            .is_some_and(|surface| surface.placement.reference.display == *display as usize),
        ContextMenuAction::ToggleLayerRender { target } => state
            .active_theme
            .as_ref()
            .and_then(|theme| {
                let effective = theme_engine::apply_mouse_action_overrides(
                    theme,
                    &state.mouse_action_overrides,
                );
                effective
                    .surfaces
                    .iter()
                    .position(|surface| surface.id.eq_ignore_ascii_case(target))
                    .map(|surface_index| {
                        let runtime = theme_runtime_for_surface(
                            &effective,
                            surface_index,
                            theme_runtime_from_state(state),
                        );
                        theme_engine::surface_should_render(
                            &effective,
                            surface_index,
                            state.data.as_ref(),
                            runtime,
                        )
                    })
            })
            .unwrap_or(false),
        _ => false,
    };
    let disabled = matches!(
        action,
        ContextMenuAction::CheckForUpdates
            if matches!(state.update_status, UpdateStatus::Checking | UpdateStatus::Applying)
    ) || matches!(
        action,
        ContextMenuAction::LayerActions { .. } | ContextMenuAction::ToggleLayerRender { .. }
    ) && origin.is_none()
        && state.active_theme.is_none();
    match (checked, disabled) {
        (true, true) => MF_CHECKED | MF_GRAYED,
        (true, false) => MF_CHECKED,
        (false, true) => MF_GRAYED,
        (false, false) => MENU_ITEM_FLAGS(0),
    }
}

pub(super) fn context_menu_action_origin(
    origin: Option<(usize, String)>,
) -> Option<(usize, String)> {
    if origin.is_some() {
        return origin;
    }
    lock_state().as_ref().and_then(|state| {
        state
            .active_theme
            .as_ref()
            .and_then(|theme| theme.surfaces.first())
            .map(|surface| (0, surface.id.clone()))
    })
}

pub(super) fn context_menu_widget_origin(theme: &ThemeDocument) -> Option<(usize, String)> {
    theme
        .surfaces
        .iter()
        .enumerate()
        .find(|(_, surface)| surface.placement.nest != SurfaceNest::TrayIcon)
        .map(|(index, surface)| (index, surface.id.clone()))
}

/// True when the taskbar covering this display owns a notification area.
///
/// Only the primary taskbar has one. Anchoring a surface to `SystemTray` on a
/// display without it resolves to the taskbar's own origin, and a right-edge
/// alignment then pushes the surface a full width past the screen edge, where
/// it is invisible. Callers use this to pick an anchor the display can honour.
fn display_has_system_tray(display: usize) -> bool {
    let Some(monitor) = native_interop::find_monitors().get(display).copied() else {
        return false;
    };
    native_interop::find_taskbars().into_iter().any(|taskbar| {
        let centre_x = (taskbar.rect.left + taskbar.rect.right) / 2;
        let centre_y = (taskbar.rect.top + taskbar.rect.bottom) / 2;
        centre_x >= monitor.rect.left
            && centre_x < monitor.rect.right
            && centre_y >= monitor.rect.top
            && centre_y < monitor.rect.bottom
            && native_interop::find_child_window(taskbar.hwnd, "TrayNotifyWnd").is_some()
    })
}

/// Anchor to use on a display, given whether its taskbar has a notification area.
///
/// A surface is positioned at `anchor point - surface factor * width`, so the
/// reference region and the surface's own alignment only make sense chosen
/// together. Beside a notification area, the widget's right edge goes against
/// the tray's left edge. Without one, that same pairing would measure from the
/// taskbar's left edge and land a full width off screen, so the widget's left
/// edge goes against the taskbar's left edge instead.
pub(super) fn placement_for_display(has_system_tray: bool) -> (ReferenceRegion, HorizontalAnchor) {
    if has_system_tray {
        (ReferenceRegion::SystemTray, HorizontalAnchor::Right)
    } else {
        (ReferenceRegion::Taskbar, HorizontalAnchor::Left)
    }
}

/// Move every surface of the active theme to `display` and persist it.
fn apply_display_choice(hwnd: HWND, display: usize) {
    let Some(mut theme) = lock_state()
        .as_ref()
        .and_then(|state| state.active_theme.clone())
    else {
        return;
    };

    let (region, surface_horizontal) = placement_for_display(display_has_system_tray(display));

    let mut changed = false;
    for surface in &mut theme.surfaces {
        let placement = &mut surface.placement;
        if placement.reference.display != display
            || placement.reference.region != region
            || placement.horizontal != HorizontalAnchor::Left
            || placement.surface_horizontal != Some(surface_horizontal)
        {
            placement.reference.display = display;
            placement.reference.region = region;
            placement.horizontal = HorizontalAnchor::Left;
            placement.surface_horizontal = Some(surface_horizontal);
            // A drag-adjusted offset belongs to the old anchor, not this one.
            placement.offset_x = 0;
            changed = true;
        }
    }
    if !changed {
        return;
    }

    match theme_engine::save_theme(&theme) {
        Ok(path) => {
            let mut settings = load_settings();
            settings.active_theme_path = Some(path.to_string_lossy().into_owned());
            save_settings_or_log(&settings, "unable to persist the chosen screen");
            if let Err(error) = apply_custom_theme(hwnd, true, Some(path), Some(theme)) {
                diagnose::log(format!("screen change could not be applied: {error}"));
            }
            position_at_taskbar();
            render_layered();
            diagnose::log(format!("widget moved to display {display} ({region:?})"));
        }
        Err(error) => {
            // Built-in themes are read-only; the user has to duplicate first.
            diagnose::log(format!("screen change could not be saved: {error}"));
        }
    }
}

pub(super) fn execute_context_menu_action(
    hwnd: HWND,
    action: ContextMenuAction,
    origin: Option<(usize, String)>,
) {
    let static_command = match &action {
        ContextMenuAction::OpenDashboard => Some(IDM_DASHBOARD),
        ContextMenuAction::Refresh => Some(1),
        ContextMenuAction::SetUpdateFrequency { seconds } => match *seconds {
            POLL_1_MIN_SECONDS => Some(IDM_FREQ_1MIN),
            POLL_5_MIN_SECONDS => Some(IDM_FREQ_5MIN),
            POLL_15_MIN_SECONDS => Some(IDM_FREQ_15MIN),
            POLL_1_HOUR_SECONDS => Some(IDM_FREQ_1HOUR),
            _ => None,
        },
        ContextMenuAction::ToggleProvider { provider } => {
            Some(provider.descriptor().native_menu_command_id)
        }
        ContextMenuAction::ToggleStartup => Some(IDM_START_WITH_WINDOWS),
        ContextMenuAction::SetLanguage { language } => {
            if language.eq_ignore_ascii_case("system") {
                Some(IDM_LANG_SYSTEM)
            } else {
                LanguageId::from_code(language).map(language_menu_command_id)
            }
        }
        ContextMenuAction::CheckForUpdates => Some(IDM_VERSION_ACTION),
        ContextMenuAction::Exit => Some(2),
        ContextMenuAction::ToggleWidget
        | ContextMenuAction::LegacyResetPosition
        | ContextMenuAction::SetDisplay { .. }
        | ContextMenuAction::ToggleLayerRender { .. }
        | ContextMenuAction::LayerActions { .. }
        | ContextMenuAction::OpenUrl { .. } => None,
    };
    if let Some(command) = static_command {
        unsafe {
            let _ = PostMessageW(hwnd, WM_COMMAND, WPARAM(command as usize), LPARAM(0));
        }
        return;
    }
    match action {
        ContextMenuAction::ToggleWidget => {
            let target = lock_state()
                .as_ref()
                .and_then(|state| state.active_theme.as_ref())
                .and_then(context_menu_widget_origin);
            if let Some((surface_index, root_id)) = target {
                let _ =
                    execute_mouse_action_source(surface_index, &root_id, "toggle(self, render)");
            }
        }
        ContextMenuAction::ToggleLayerRender { target } => {
            let Some((surface_index, self_id)) = context_menu_action_origin(origin) else {
                return;
            };
            let target = target.replace('\\', "\\\\").replace('"', "\\\"");
            let source = format!("toggle(\"{target}\", render)");
            let _ = execute_mouse_action_source(surface_index, &self_id, &source);
        }
        ContextMenuAction::LayerActions { actions } => {
            let Some((surface_index, self_id)) = context_menu_action_origin(origin) else {
                return;
            };
            let _ = execute_mouse_action_source(surface_index, &self_id, &actions);
        }
        ContextMenuAction::SetDisplay { display } => {
            apply_display_choice(hwnd, display as usize);
        }
        ContextMenuAction::OpenUrl { url } if context_menu::supported_url(&url) => unsafe {
            let operation = native_interop::wide_str("open");
            let url = native_interop::wide_str(url.trim());
            let result = ShellExecuteW(
                hwnd,
                PCWSTR::from_raw(operation.as_ptr()),
                PCWSTR::from_raw(url.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
            if result.0 as isize <= 32 {
                diagnose::log("context menu URL could not be opened");
            }
        },
        _ => {}
    }
}
