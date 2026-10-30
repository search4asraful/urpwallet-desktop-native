use std::cell::RefCell;
use std::time::Duration;
use slint::{ComponentHandle, Timer, TimerMode};
use crate::MainWindow;

thread_local! {
    static TOAST_TIMER: RefCell<Option<Timer>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastType {
    Success,
    Error,
    Info,
}

impl ToastType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToastType::Success => "success",
            ToastType::Error => "error",
            ToastType::Info => "info",
        }
    }
}

/// Displays a floating flash popup / toast notification.
/// Automatically dismisses after 4 seconds, or when dismissed by user.
pub fn show_toast(ui: &MainWindow, toast_type: ToastType, message: impl AsRef<str>) {
    let msg_str = message.as_ref();
    let clean_msg = msg_str.strip_prefix("✓ ").unwrap_or(msg_str).trim();

    ui.set_toast_type(toast_type.as_str().into());
    ui.set_toast_message(clean_msg.into());
    ui.set_toast_visible(true);

    TOAST_TIMER.with(|cell| {
        let timer = Timer::default();
        let ui_weak = ui.as_weak();
        timer.start(TimerMode::SingleShot, Duration::from_millis(4000), move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_toast_visible(false);
            }
        });
        *cell.borrow_mut() = Some(timer);
    });
}

/// Shows a success flash message.
pub fn show_success(ui: &MainWindow, message: impl AsRef<str>) {
    show_toast(ui, ToastType::Success, message);
}

/// Shows an error flash message.
pub fn show_error(ui: &MainWindow, message: impl AsRef<str>) {
    show_toast(ui, ToastType::Error, message);
}

/// Shows an informational flash message.
pub fn show_info(ui: &MainWindow, message: impl AsRef<str>) {
    show_toast(ui, ToastType::Info, message);
}

/// Dismisses any currently visible toast and cancels auto-hide timer.
pub fn dismiss_toast(ui: &MainWindow) {
    ui.set_toast_visible(false);
    TOAST_TIMER.with(|cell| {
        if let Some(timer) = cell.borrow_mut().take() {
            timer.stop();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_types() {
        assert_eq!(ToastType::Success.as_str(), "success");
        assert_eq!(ToastType::Error.as_str(), "error");
        assert_eq!(ToastType::Info.as_str(), "info");
    }
}
