//! Native geometry adaptation; pure work-area selection lives in settings.
use crate::settings::{constrain_window, Rect, SettingsStore};
use std::sync::Mutex;
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

pub(crate) fn window_rect(window: &WebviewWindow) -> tauri::Result<Rect> {
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    Ok(Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}
pub(crate) fn normal_window(window: &WebviewWindow) -> bool {
    !window.is_minimized().unwrap_or(true)
        && !window.is_maximized().unwrap_or(true)
        && !window.is_fullscreen().unwrap_or(true)
}
pub(crate) fn remember_window(window: &WebviewWindow, store: &SettingsStore) {
    if normal_window(window) {
        if let Ok(rect) = window_rect(window) {
            if rect.width > 0 && rect.height > 0 && store.snapshot().window != Some(rect) {
                store.update(|s| s.window = Some(rect));
            }
        }
    }
}

// Display topology is checked only on native window events; no idle polling.
#[derive(Default)]
pub(crate) struct Displays(pub(crate) Mutex<Vec<Rect>>);
pub(crate) fn display_work_areas(window: &WebviewWindow) -> tauri::Result<Vec<Rect>> {
    Ok(window
        .available_monitors()?
        .iter()
        .map(|monitor| {
            let area = monitor.work_area();
            Rect {
                x: area.position.x,
                y: area.position.y,
                width: area.size.width,
                height: area.size.height,
            }
        })
        .collect())
}
pub(crate) fn displays_changed(window: &WebviewWindow, displays: &Displays) -> tauri::Result<bool> {
    let current = display_work_areas(window)?;
    let mut previous = displays.0.lock().unwrap();
    let changed = *previous != current;
    *previous = current;
    Ok(changed)
}

pub(crate) fn fit_window(window: &WebviewWindow, requested: Rect) -> tauri::Result<()> {
    let outer = window.outer_size()?;
    let inner = window.inner_size()?;
    let frame = (
        outer.width.saturating_sub(inner.width),
        outer.height.saturating_sub(inner.height),
    );
    let screens: Vec<_> = window
        .available_monitors()?
        .into_iter()
        .map(|monitor| {
            let area = monitor.work_area();
            (
                Rect {
                    x: area.position.x,
                    y: area.position.y,
                    width: area.size.width,
                    height: area.size.height,
                },
                (
                    (900.0 * monitor.scale_factor()) as u32 + frame.0,
                    (600.0 * monitor.scale_factor()) as u32 + frame.1,
                ),
            )
        })
        .collect();
    if screens.is_empty() {
        return Ok(());
    }
    let fitted = constrain_window(requested, &screens);
    let minimum = constrain_window(
        Rect {
            width: 1,
            height: 1,
            ..fitted
        },
        &screens,
    );
    window.set_min_size(Some(PhysicalSize::new(
        minimum.width.saturating_sub(frame.0).max(1),
        minimum.height.saturating_sub(frame.1).max(1),
    )))?;
    if fitted.width != outer.width || fitted.height != outer.height {
        window.set_size(PhysicalSize::new(
            fitted.width.saturating_sub(frame.0).max(1),
            fitted.height.saturating_sub(frame.1).max(1),
        ))?;
    }
    if window.outer_position()? != PhysicalPosition::new(fitted.x, fitted.y) {
        window.set_position(PhysicalPosition::new(fitted.x, fitted.y))?;
    }
    Ok(())
}
