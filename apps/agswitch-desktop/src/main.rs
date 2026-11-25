#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod phosphor;
mod tray;
mod view;

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agswitch_engine::{Engine, EngineDefaults, ProcessPort, TranscriptPort};
use agswitch_integrations::{
    OsProcessAdapter, OsTranscriptAdapter, OsWorkspaceLinkAdapter, SystemClockAdapter,
    default_antigravity_bin_path, default_data_dir, default_profiles_root_dir,
    open_directory_in_file_manager, primary_home_dir, sandbox,
};
use agswitch_primitives::RuntimeOs;
use agswitch_spec::{CallerContext, CommandRequest, CommandResponse, DashboardSnapshot};
use agswitch_store::JsonFileStore;
use gpui::{
    App, AppContext, Application, Bounds, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, point, px, size,
};

use crate::tray::{TrayAction, TrayAnchorRect, TrayController};
use crate::view::AgSwitchView;

pub const FULL_POPOVER_W: f32 = 680.0;
pub const FULL_POPOVER_H: f32 = 524.0;
pub const COMPACT_POPOVER_W: f32 = 320.0;
pub const COMPACT_POPOVER_H: f32 = 420.0;

struct PopoverSlot {
    handle: Option<WindowHandle<AgSwitchView>>,
    last_toggle: Instant,
}

fn main() -> ExitCode {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(rt) => Arc::new(rt),
        Err(err) => {
            eprintln!("failed to initialize async runtime: {err}");
            return ExitCode::FAILURE;
        }
    };

    let args: Vec<String> = std::env::args().collect();
    let is_sandbox = args.iter().any(|a| a == "--sandbox" || a == "--demo")
        || std::env::var("AGSWITCH_SANDBOX")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let demo_page = if is_sandbox {
        args.windows(2)
            .find(|w| w.first().is_some_and(|a| a == "--demo-page"))
            .and_then(|w| w.get(1).cloned())
    } else {
        None
    };

    let (data_dir, profiles_root, engine) = if is_sandbox {
        let base = sandbox::default_sandbox_dir();
        let paths = match sandbox::ensure_seeded(&base) {
            Ok(p) => p,
            Err(err) => {
                eprintln!("failed to initialize sandbox: {err}");
                return ExitCode::FAILURE;
            }
        };
        let proc_adapter: Arc<dyn ProcessPort> =
            Arc::new(sandbox::SandboxProcessAdapter::new(paths.running_state_file));
        let tx_adapter: Arc<dyn TranscriptPort> = Arc::new(sandbox::SandboxTranscriptAdapter);
        let engine = Arc::new(Engine::new(
            EngineDefaults {
                profiles_root_dir: paths.profiles_root.to_string_lossy().into_owned(),
                antigravity_bin_path: "/sandbox/Antigravity".to_owned(),
                runtime_os: RuntimeOs::current(),
            },
            Arc::new(JsonFileStore::new(paths.data_dir.clone())),
            proc_adapter,
            Arc::new(OsWorkspaceLinkAdapter::new(paths.fake_home)),
            tx_adapter,
            Arc::new(SystemClockAdapter),
        ));
        (paths.data_dir, paths.profiles_root, engine)
    } else {
        let data_dir = default_data_dir();
        let profiles_root = default_profiles_root_dir(&data_dir);
        let primary_home = primary_home_dir();
        let engine = Arc::new(Engine::new(
            EngineDefaults {
                profiles_root_dir: profiles_root.to_string_lossy().into_owned(),
                antigravity_bin_path: default_antigravity_bin_path(),
                runtime_os: RuntimeOs::current(),
            },
            Arc::new(JsonFileStore::new(data_dir.clone())),
            Arc::new(OsProcessAdapter::new()),
            Arc::new(OsWorkspaceLinkAdapter::new(primary_home.clone())),
            Arc::new(OsTranscriptAdapter::new(primary_home)),
            Arc::new(SystemClockAdapter),
        ));
        (data_dir, profiles_root, engine)
    };

    let ui_prefs_path = data_dir.join("ui-prefs.json");
    let shared_snap: Arc<Mutex<Option<DashboardSnapshot>>> = Arc::new(Mutex::new(None));

    Application::new().run(move |cx: &mut App| {
        // Initialize top Menu Bar (macOS) / System Tray (Windows) icon on main UI thread
        let tray = Rc::new(RefCell::new(TrayController::try_create()));
        let popover_slot = Rc::new(RefCell::new(PopoverSlot {
            handle: None,
            last_toggle: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
        }));

        // Open initial Samo-style menu-bar popover directly underneath the tray icon
        let initial_anchor = tray.borrow().as_ref().and_then(TrayController::anchor_rect);
        open_popover(
            cx,
            initial_anchor,
            &popover_slot,
            engine.clone(),
            rt.clone(),
            shared_snap.clone(),
            ui_prefs_path.clone(),
            is_sandbox,
            demo_page.clone(),
        );

        // App-level tray & background menu sync loop (stays alive even while popover is closed)
        let tray_loop = tray.clone();
        let slot_loop = popover_slot.clone();
        let engine_loop = engine.clone();
        let rt_loop = rt.clone();
        let snap_loop = shared_snap.clone();
        let profiles_dir_loop = profiles_root.clone();
        let prefs_loop = ui_prefs_path.clone();

        cx.spawn(async move |cx| {
            let mut ticks: u32 = 0;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(65))
                    .await;
                ticks = ticks.wrapping_add(1);

                // Every ~2s sync the native tray menu tooltip & items with latest snapshot
                if ticks.is_multiple_of(30) {
                    let eng = engine_loop.clone();
                    let rt_bg = rt_loop.clone();
                    let snap_bg = snap_loop.clone();
                    let refreshed = cx
                        .background_executor()
                        .spawn(async move {
                            rt_bg.block_on(eng.execute(
                                CommandRequest::GetDashboard,
                                CallerContext::local_interactive(),
                            ))
                        })
                        .await;
                    if let Ok(CommandResponse::Dashboard(dash)) = refreshed {
                        if let Ok(mut guard) = snap_bg.lock() {
                            *guard = Some(dash.clone());
                        }
                        let _ = cx.update(|_cx| {
                            if let Some(t) = tray_loop.borrow_mut().as_mut() {
                                t.sync_snapshot(&dash);
                            }
                        });
                    }
                }

                let actions: Vec<TrayAction> = cx
                    .update(|_cx| {
                        tray_loop
                            .borrow()
                            .as_ref()
                            .map_or_else(Vec::new, TrayController::drain_actions)
                    })
                    .unwrap_or_default();

                for action in actions {
                    match action {
                        TrayAction::TogglePanel(anchor) => {
                            let _ = cx.update(|cx| {
                                toggle_popover(
                                    cx,
                                    anchor,
                                    &slot_loop,
                                    engine_loop.clone(),
                                    rt_loop.clone(),
                                    snap_loop.clone(),
                                    prefs_loop.clone(),
                                    is_sandbox,
                                );
                            });
                        }
                        TrayAction::SwitchNext => {
                            let eng = engine_loop.clone();
                            let rt_bg = rt_loop.clone();
                            let _ = cx
                                .background_executor()
                                .spawn(async move {
                                    let _ = rt_bg.block_on(eng.execute(
                                        CommandRequest::SwitchNext,
                                        CallerContext::local_interactive(),
                                    ));
                                })
                                .await;
                        }
                        TrayAction::SwitchAccount(profile_id) => {
                            let eng = engine_loop.clone();
                            let rt_bg = rt_loop.clone();
                            let _ = cx
                                .background_executor()
                                .spawn(async move {
                                    let _ = rt_bg.block_on(eng.execute(
                                        CommandRequest::SwitchToProfile { profile_id },
                                        CallerContext::local_interactive(),
                                    ));
                                })
                                .await;
                        }
                        TrayAction::OpenProfilesFolder => {
                            let _ = open_directory_in_file_manager(&PathBuf::from(
                                &profiles_dir_loop,
                            ));
                        }
                        TrayAction::Quit => {
                            let _ = cx.update(|cx| cx.quit());
                            return;
                        }
                    }
                }
            }
        })
        .detach();
    });

    ExitCode::SUCCESS
}

#[allow(clippy::too_many_arguments)]
fn toggle_popover(
    cx: &mut App,
    anchor: Option<TrayAnchorRect>,
    slot: &Rc<RefCell<PopoverSlot>>,
    engine: Arc<Engine>,
    rt: Arc<tokio::runtime::Runtime>,
    shared_snap: Arc<Mutex<Option<DashboardSnapshot>>>,
    ui_prefs_path: PathBuf,
    is_sandbox: bool,
) {
    let existing = slot.borrow().handle;
    if let Some(handle) = existing {
        // If window still exists, close it (toggle off -> 0% CPU while closed!)
        let was_open = cx
            .update_window(handle.into(), |_view, window, _cx| {
                window.remove_window();
            })
            .is_ok();
        slot.borrow_mut().handle = None;
        slot.borrow_mut().last_toggle = Instant::now();
        if was_open {
            return;
        }
    }

    open_popover(
        cx,
        anchor,
        slot,
        engine,
        rt,
        shared_snap,
        ui_prefs_path,
        is_sandbox,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn open_popover(
    cx: &mut App,
    anchor: Option<TrayAnchorRect>,
    slot: &Rc<RefCell<PopoverSlot>>,
    engine: Arc<Engine>,
    rt: Arc<tokio::runtime::Runtime>,
    shared_snap: Arc<Mutex<Option<DashboardSnapshot>>>,
    ui_prefs_path: PathBuf,
    is_sandbox: bool,
    demo_page: Option<String>,
) {
    let is_compact = match demo_page.as_deref() {
        Some("compact" | "compact-light") => true,
        Some(_) => false,
        None => read_initial_compact_pref(&ui_prefs_path),
    };
    let bounds = compute_popover_bounds(cx, anchor, is_compact);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // Borderless Samo-style popover: NO system titlebar, NO traffic lights!
        titlebar: None,
        window_background: WindowBackgroundAppearance::Transparent,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        ..WindowOptions::default()
    };

    let slot_for_close = slot.clone();
    match cx.open_window(options, move |window, cx| {
        cx.new(|cx| {
            AgSwitchView::new(
                window,
                cx,
                engine,
                rt,
                shared_snap,
                ui_prefs_path,
                is_sandbox,
                demo_page,
                move || {
                    slot_for_close.borrow_mut().handle = None;
                },
            )
        })
    }) {
        Ok(handle) => {
            slot.borrow_mut().handle = Some(handle);
            slot.borrow_mut().last_toggle = Instant::now();
            cx.activate(true);
        }
        Err(err) => {
            eprintln!("failed to open GPUI popover: {err}");
        }
    }
}

fn read_initial_compact_pref(ui_prefs_path: &PathBuf) -> bool {
    std::fs::read_to_string(ui_prefs_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("compact_mode").and_then(serde_json::Value::as_bool))
        .unwrap_or(false)
}

/// Position popover centered directly beneath the macOS Menu Bar icon (or top-right fallback on startup).
fn compute_popover_bounds(
    cx: &App,
    anchor: Option<TrayAnchorRect>,
    is_compact: bool,
) -> Bounds<gpui::Pixels> {
    let pop_w = if is_compact {
        COMPACT_POPOVER_W
    } else {
        FULL_POPOVER_W
    };
    let pop_h = if is_compact {
        COMPACT_POPOVER_H
    } else {
        FULL_POPOVER_H
    };

    let (disp_x, disp_y, disp_w, disp_h) = if let Some(display) = cx.primary_display() {
        let b = display.bounds();
        let x: f32 = b.origin.x.into();
        let y: f32 = b.origin.y.into();
        let w: f32 = b.size.width.into();
        let h: f32 = b.size.height.into();
        (x, y, w, h)
    } else {
        (0.0, 0.0, 1440.0, 900.0)
    };

    let mut origin_x = disp_x + disp_w - pop_w - 14.0;
    let mut origin_y = if cfg!(target_os = "windows") {
        (disp_y + disp_h - pop_h - 54.0).max(disp_y + 12.0)
    } else {
        disp_y + 34.0
    };

    if let Some(rect) = anchor
        && rect.width > 1.0
        && rect.height > 1.0
        && (rect.x > 1.0 || rect.y > 1.0)
    {
        // tray-icon returns physical pixels on macOS Retina (menu bar height >= 42px physical -> 2x)
        let scale = if rect.height >= 42.0 || rect.x > f64::from(disp_w) {
            2.0_f64
        } else {
            1.0_f64
        };
        let logical_cx = f64_to_f32((rect.x + rect.width * 0.5) / scale);
        let logical_bottom = f64_to_f32((rect.y + rect.height) / scale);

        origin_x = logical_cx - pop_w * 0.5;
        if cfg!(target_os = "windows") {
            origin_y = (disp_y + disp_h - pop_h - 54.0).max(disp_y + 12.0);
        } else {
            origin_y = (logical_bottom + 6.0).clamp(disp_y + 28.0, disp_y + 46.0);
        }
    }

    let min_x = disp_x + 10.0;
    let max_x = (disp_x + disp_w - pop_w - 10.0).max(min_x);
    origin_x = origin_x.clamp(min_x, max_x);

    Bounds::new(point(px(origin_x), px(origin_y)), size(px(pop_w), px(pop_h)))
}

/// Safe `f64 -> f32` screen coordinate conversion without `as` casts.
fn f64_to_f32(val: f64) -> f32 {
    let clamped = val.clamp(-16_000.0, 16_000.0);
    let negative = clamped < 0.0;
    let abs_scaled = clamped.abs() * 10.0;
    let mut lo: u32 = 0;
    let mut hi: u32 = 160_000;
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if f64::from(mid) <= abs_scaled {
            lo = mid;
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    let tenths = lo;
    let mag = f32::from(u16::try_from(tenths / 10).unwrap_or(u16::MAX))
        + f32::from(u8::try_from(tenths % 10).unwrap_or(0)) * 0.1;
    if negative { -mag } else { mag }
}
