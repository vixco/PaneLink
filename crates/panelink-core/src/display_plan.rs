use serde::{Deserialize, Serialize};

use crate::{RemoteScreen, ScaleMode, ScreenRole, ScreenStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayTopology {
    pub pc_id: String,
    pub pc_name: String,
    pub displays: Vec<TargetDisplay>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetDisplay {
    pub id: String,
    pub name: String,
    pub role: TargetDisplayRole,
    pub native_mode: DisplayMode,
    pub current_mode: DisplayMode,
    pub supported_modes: Vec<DisplayMode>,
    pub bounds: DisplayRect,
    pub attached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetDisplayRole {
    Primary,
    Extended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDisplay {
    pub id: String,
    pub name: String,
    pub native_mode: DisplayMode,
    pub current_mode: DisplayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySessionPlan {
    pub id: String,
    pub peer_id: String,
    pub windows_pc: DisplayTopology,
    pub screens: Vec<PlannedScreen>,
    pub rollback_snapshot: RollbackSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedScreen {
    pub source_display: SourceDisplay,
    pub target_display: TargetDisplay,
    pub selected_mode: DisplayMode,
    pub fitted_resolution: FittedResolution,
    pub remote_screen: RemoteScreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FittedResolution {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u16,
    pub insets: ScaleInsets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleInsets {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackSnapshot {
    pub id: String,
    pub peer_id: String,
    pub reason: RollbackReason,
    pub local_layout: DisplayLayout,
    pub remote_layout: DisplayLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RollbackReason {
    Disconnect,
    CrashRecovery,
    UserCancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayLayout {
    pub pc_id: String,
    pub displays: Vec<DisplayLayoutEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayLayoutEntry {
    pub display_id: String,
    pub mode: DisplayMode,
    pub bounds: DisplayRect,
    pub primary: bool,
    pub attached: bool,
}

pub fn plan_add_screen(
    plan_id: impl Into<String>,
    peer_id: impl Into<String>,
    windows_pc: DisplayTopology,
    source_display: SourceDisplay,
    target_display_id: &str,
) -> Result<DisplaySessionPlan, DisplayPlanError> {
    let plan_id = plan_id.into();
    let peer_id = peer_id.into();
    let target_display = windows_pc
        .displays
        .iter()
        .find(|display| display.id == target_display_id && display.attached)
        .cloned()
        .ok_or(DisplayPlanError::TargetDisplayUnavailable)?;
    let selected_mode = auto_fit_mode(source_display.current_mode, &target_display)
        .ok_or(DisplayPlanError::NoSupportedMode)?;
    let fitted_resolution = fit_resolution(source_display.current_mode, selected_mode);
    let remote_screen = RemoteScreen {
        id: format!("screen-{}", target_display.id),
        name: target_display.name.clone(),
        role: match target_display.role {
            TargetDisplayRole::Primary => ScreenRole::Primary,
            TargetDisplayRole::Extended => ScreenRole::Extended,
        },
        source_display: source_display.name.clone(),
        target_display: target_display.name.clone(),
        native_resolution: format_mode(target_display.native_mode),
        fitted_resolution: format_fitted_resolution(fitted_resolution),
        scale_mode: ScaleMode::AutoFit,
        status: ScreenStatus::Ready,
    };

    let rollback_snapshot = RollbackSnapshot {
        id: format!("{plan_id}-rollback"),
        peer_id: peer_id.clone(),
        reason: RollbackReason::Disconnect,
        local_layout: DisplayLayout {
            pc_id: "local".into(),
            displays: vec![DisplayLayoutEntry {
                display_id: source_display.id.clone(),
                mode: source_display.current_mode,
                bounds: DisplayRect {
                    x: 0,
                    y: 0,
                    width: source_display.current_mode.width,
                    height: source_display.current_mode.height,
                },
                primary: true,
                attached: true,
            }],
        },
        remote_layout: layout_from_topology(&windows_pc),
    };

    Ok(DisplaySessionPlan {
        id: plan_id,
        peer_id,
        windows_pc,
        screens: vec![PlannedScreen {
            source_display,
            target_display,
            selected_mode,
            fitted_resolution,
            remote_screen,
        }],
        rollback_snapshot,
    })
}

pub fn auto_fit_mode(
    source_mode: DisplayMode,
    target_display: &TargetDisplay,
) -> Option<DisplayMode> {
    target_display
        .supported_modes
        .iter()
        .copied()
        .filter(|mode| mode.width <= target_display.native_mode.width)
        .filter(|mode| mode.height <= target_display.native_mode.height)
        .max_by_key(|mode| auto_fit_score(source_mode, *mode))
}

pub fn fit_resolution(source_mode: DisplayMode, selected_mode: DisplayMode) -> FittedResolution {
    let selected_width = u64::from(selected_mode.width);
    let selected_height = u64::from(selected_mode.height);
    let source_width = u64::from(source_mode.width);
    let source_height = u64::from(source_mode.height);

    let (width, height) = if selected_width * source_height <= selected_height * source_width {
        let height = selected_width * source_height / source_width;
        (selected_width, height)
    } else {
        let width = selected_height * source_width / source_height;
        (width, selected_height)
    };

    let horizontal = selected_width.saturating_sub(width) as u32;
    let vertical = selected_height.saturating_sub(height) as u32;

    FittedResolution {
        width: width as u32,
        height: height as u32,
        refresh_hz: selected_mode.refresh_hz,
        insets: ScaleInsets {
            left: horizontal / 2,
            right: horizontal - horizontal / 2,
            top: vertical / 2,
            bottom: vertical - vertical / 2,
        },
    }
}

pub fn layout_from_topology(topology: &DisplayTopology) -> DisplayLayout {
    DisplayLayout {
        pc_id: topology.pc_id.clone(),
        displays: topology
            .displays
            .iter()
            .map(|display| DisplayLayoutEntry {
                display_id: display.id.clone(),
                mode: display.current_mode,
                bounds: display.bounds,
                primary: display.role == TargetDisplayRole::Primary,
                attached: display.attached,
            })
            .collect(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayPlanError {
    TargetDisplayUnavailable,
    NoSupportedMode,
}

fn auto_fit_score(source_mode: DisplayMode, mode: DisplayMode) -> (u64, u64, u16) {
    (
        u64::MAX - aspect_delta(source_mode, mode),
        u64::from(mode.width) * u64::from(mode.height),
        mode.refresh_hz,
    )
}

fn aspect_delta(source_mode: DisplayMode, mode: DisplayMode) -> u64 {
    let source_gcd = gcd(source_mode.width, source_mode.height);
    let mode_gcd = gcd(mode.width, mode.height);
    let source_width = source_mode.width / source_gcd;
    let source_height = source_mode.height / source_gcd;
    let mode_width = mode.width / mode_gcd;
    let mode_height = mode.height / mode_gcd;
    let lhs = i128::from(source_width) * i128::from(mode_height);
    let rhs = i128::from(mode_width) * i128::from(source_height);
    lhs.abs_diff(rhs) as u64
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }

    left.max(1)
}

fn format_mode(mode: DisplayMode) -> String {
    format!("{} x {} @ {} Hz", mode.width, mode.height, mode.refresh_hz)
}

fn format_fitted_resolution(resolution: FittedResolution) -> String {
    format!(
        "{} x {} @ {} Hz",
        resolution.width, resolution.height, resolution.refresh_hz
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_fit_prefers_matching_aspect_before_area() {
        let target = target_display(vec![
            mode(2560, 1440, 120),
            mode(1920, 1200, 60),
            mode(1920, 1080, 120),
        ]);

        let selected = auto_fit_mode(mode(2560, 1600, 120), &target).unwrap();

        assert_eq!(selected, mode(1920, 1200, 60));
        assert_eq!(
            fit_resolution(mode(2560, 1600, 120), selected).insets,
            zero_insets()
        );
    }

    #[test]
    fn auto_fit_letterboxes_when_no_exact_aspect_exists() {
        let target = target_display(vec![mode(2560, 1440, 120), mode(1920, 1080, 120)]);

        let selected = auto_fit_mode(mode(2560, 1600, 120), &target).unwrap();
        let fitted = fit_resolution(mode(2560, 1600, 120), selected);

        assert_eq!(selected, mode(2560, 1440, 120));
        assert_eq!(fitted.width, 2304);
        assert_eq!(fitted.height, 1440);
        assert_eq!(
            fitted.insets,
            ScaleInsets {
                left: 128,
                top: 0,
                right: 128,
                bottom: 0,
            }
        );
    }

    #[test]
    fn add_screen_plan_captures_rollback_before_changes() {
        let topology = DisplayTopology {
            pc_id: "windows-desk".into(),
            pc_name: "Windows Desk".into(),
            displays: vec![
                TargetDisplay {
                    id: "display-1".into(),
                    name: "Desk left".into(),
                    role: TargetDisplayRole::Primary,
                    native_mode: mode(2560, 1440, 120),
                    current_mode: mode(2560, 1440, 120),
                    supported_modes: vec![mode(2560, 1440, 120)],
                    bounds: rect(0, 0, 2560, 1440),
                    attached: true,
                },
                TargetDisplay {
                    id: "display-2".into(),
                    name: "Desk right".into(),
                    role: TargetDisplayRole::Extended,
                    native_mode: mode(1920, 1080, 60),
                    current_mode: mode(1920, 1080, 60),
                    supported_modes: vec![mode(1920, 1080, 60)],
                    bounds: rect(2560, 0, 1920, 1080),
                    attached: true,
                },
            ],
        };
        let source = SourceDisplay {
            id: "macbook".into(),
            name: "MacBook display".into(),
            native_mode: mode(2560, 1600, 120),
            current_mode: mode(2560, 1600, 120),
        };

        let plan =
            plan_add_screen("plan-1", "windows-desk", topology, source, "display-2").unwrap();

        assert_eq!(plan.screens.len(), 1);
        assert_eq!(plan.rollback_snapshot.remote_layout.displays.len(), 2);
        assert_eq!(
            plan.rollback_snapshot.remote_layout.displays[1],
            DisplayLayoutEntry {
                display_id: "display-2".into(),
                mode: mode(1920, 1080, 60),
                bounds: rect(2560, 0, 1920, 1080),
                primary: false,
                attached: true,
            }
        );
        assert_eq!(
            plan.rollback_snapshot.local_layout.displays[0].mode,
            mode(2560, 1600, 120)
        );
        assert_eq!(plan.rollback_snapshot.reason, RollbackReason::Disconnect);
    }

    fn target_display(supported_modes: Vec<DisplayMode>) -> TargetDisplay {
        TargetDisplay {
            id: "display-1".into(),
            name: "Desk display".into(),
            role: TargetDisplayRole::Primary,
            native_mode: mode(2560, 1440, 120),
            current_mode: mode(2560, 1440, 120),
            supported_modes,
            bounds: rect(0, 0, 2560, 1440),
            attached: true,
        }
    }

    fn mode(width: u32, height: u32, refresh_hz: u16) -> DisplayMode {
        DisplayMode {
            width,
            height,
            refresh_hz,
        }
    }

    fn rect(x: i32, y: i32, width: u32, height: u32) -> DisplayRect {
        DisplayRect {
            x,
            y,
            width,
            height,
        }
    }

    fn zero_insets() -> ScaleInsets {
        ScaleInsets {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        }
    }
}
