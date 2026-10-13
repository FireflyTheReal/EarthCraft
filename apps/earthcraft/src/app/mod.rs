//! Desktop state and orchestration; widgets live in focused child modules.
use earthcraft_core::{camera::Cam, language::Lang, placemark::Placemark};
use earthcraft_data::{
    boundaries::{load_boundaries_async, BoundaryData, Patch, StatusLog},
    search::{GeoResult, Suggestion},
    settings::{load_lang, Settings},
    tiles::{Provider, TileCache},
    wayback::WaybackRelease,
};
use eframe::egui::{self, Rect};
use std::{
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
mod files;
mod globe;
mod navigation;
mod overlays;
mod rendering;
mod search;
mod ui;
mod wayback;
const INITIAL_CAMERA: Cam = Cam {
    lat: -15.0,
    lon: -50.0,
    heading: 0.0,
    alt: 14_000.0,
};
// Clamp animation/input integration after slow frames without changing original speeds.
const MAX_FRAME_DELTA_SECS: f32 = 0.1;
const PAN_SPEED_POINTS_PER_SECOND: f64 = 350.0;
const BOUNDARY_POLL_INTERVAL: Duration = Duration::from_millis(300);

type SuggestionResponse = (String, Result<Vec<Suggestion>, earthcraft_data::Error>);

struct Fly {
    a: Cam,
    b: Cam,
    t: f64,
    dur: f64,
    bump: f64,
}

pub(crate) struct App {
    runtime: earthcraft_data::runtime::ServiceRuntime,
    boundary_loader: earthcraft_data::boundaries::BoundaryLoader,
    lang: Lang,
    cam: Cam,
    fly: Option<Fly>,
    cache: Arc<TileCache>,
    provider: Provider,
    show_grid: bool,
    show_country_boundaries: bool,
    show_state_boundaries: bool,
    show_city_boundaries: bool,
    show_metro_boundaries: bool,
    show_place_labels: bool,
    boundaries: BoundaryData,
    boundaries_rx: Option<mpsc::Receiver<Patch>>,
    boundary_log: StatusLog,
    show_atmo: bool,
    show_sun: bool,
    auto_rotate: bool,

    texture: Option<egui::TextureHandle>,
    image: Option<(usize, usize, Vec<u8>)>,
    last_state: u64,
    last_gen: u64,
    last_render: Instant,
    last_interact: Instant,

    query: String,
    results: Vec<GeoResult>,
    search_rx: Option<mpsc::Receiver<Result<Vec<GeoResult>, earthcraft_data::Error>>>,
    status: String,

    placemarks: Vec<Placemark>,
    new_name: String,
    kml_path: String,
    menu_pick: Option<(f64, f64)>,

    ruler: bool,
    ruler_pts: Vec<(f64, f64)>,
    cursor: Option<(f64, f64)>,
    show_help: bool,

    settings: Settings,
    saved_settings: Settings,
    applied_ui_scale: f32,
    ui_scale_edit: f32, // slider value while dragging (committed on release)
    show_settings: bool,

    wayback_list: Vec<WaybackRelease>,
    wayback_idx: usize,
    wayback_rx: Option<mpsc::Receiver<Result<Vec<WaybackRelease>, earthcraft_data::Error>>>,
    wayback_started: bool,
    wayback_err: Option<String>,

    suggestions: Vec<Suggestion>,
    remote_sugg: Vec<Suggestion>,
    sugg_open: bool,
    sugg_sel: Option<usize>,
    sugg_last_edit: Instant,
    sugg_sent: String,
    sugg_rx: Option<mpsc::Receiver<SuggestionResponse>>,

    kml_drop_rect: Rect,
    ring_drag: bool,
}

impl App {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut vis = egui::Visuals::dark();
        vis.window_rounding = 6.0.into();
        cc.egui_ctx.set_visuals(vis);
        let runtime =
            earthcraft_data::runtime::ServiceRuntime::new().expect("start network runtime");
        let lang = load_lang();
        let settings = Settings::load();
        let boundary_log: StatusLog = Arc::new(Mutex::new(Vec::new()));
        let (boundaries_rx, boundary_loader) =
            load_boundaries_async(boundary_log.clone(), runtime.handle(), {
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            });
        App {
            lang,
            cam: INITIAL_CAMERA,
            fly: None,
            cache: TileCache::new(
                {
                    let ctx = cc.egui_ctx.clone();
                    move || ctx.request_repaint()
                },
                runtime.handle(),
            ),
            provider: Provider::Satellite,
            show_grid: false,
            show_country_boundaries: true,
            show_state_boundaries: true,
            show_city_boundaries: false,
            show_metro_boundaries: false,
            show_place_labels: true,
            boundaries: BoundaryData::default(),
            boundaries_rx: Some(boundaries_rx),
            boundary_loader,
            boundary_log,
            show_atmo: true,
            show_sun: false,
            auto_rotate: false,
            texture: None,
            image: None,
            last_state: 0,
            last_gen: u64::MAX,
            last_render: Instant::now(),
            last_interact: Instant::now(),
            query: String::new(),
            results: vec![],
            search_rx: None,
            status: lang.text("app-ready-drag-to-rotate-scroll-to-zoom").into(),
            placemarks: vec![],
            new_name: String::new(),
            kml_path: "placemarks.kml".into(),
            menu_pick: None,
            ruler: false,
            ruler_pts: vec![],
            cursor: None,
            show_help: false,
            ui_scale_edit: settings.ui_scale,
            settings: settings.clone(),
            saved_settings: settings,
            applied_ui_scale: 0.0,
            show_settings: false,
            wayback_list: vec![],
            wayback_idx: 0,
            wayback_rx: None,
            wayback_started: false,
            wayback_err: None,
            suggestions: vec![],
            remote_sugg: vec![],
            sugg_open: false,
            sugg_sel: None,
            sugg_last_edit: Instant::now(),
            sugg_sent: String::new(),
            sugg_rx: None,
            kml_drop_rect: Rect::NOTHING,
            ring_drag: false,
            runtime,
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.unstable_dt).min(MAX_FRAME_DELTA_SECS) as f64;
        if (self.settings.ui_scale - self.applied_ui_scale).abs() > 1e-4 {
            ui::apply_ui_scale(ctx, self.settings.ui_scale);
            self.applied_ui_scale = self.settings.ui_scale;
        }
        // persist settings once a slider is released
        if self.settings != self.saved_settings && !ctx.input(|i| i.pointer.any_down()) {
            self.settings.save();
            self.saved_settings = self.settings.clone();
        }
        self.handle_dropped_files(ctx);
        self.ensure_wayback(ctx);
        self.poll_wayback();
        self.poll_suggestions(ctx);
        self.poll_search();
        if let Some(rx) = self.boundaries_rx.take() {
            let mut finished = false;
            loop {
                match rx.try_recv() {
                    Ok(patch) => self.merge_patch(patch),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        finished = true;
                        break;
                    }
                }
            }
            if !finished {
                self.boundaries_rx = Some(rx);
                if self.boundary_loader.is_loading() {
                    ctx.request_repaint_after(BOUNDARY_POLL_INTERVAL);
                } // keep polling while layers load
            }
        }
        self.step_fly(dt);
        if self.fly.is_some() {
            ctx.request_repaint();
        }
        self.top_bar(ctx);
        self.bottom_bar(ctx);
        self.side_panel(ctx);
        self.settings_window(ctx);
        self.boundary_loader.request_visible_layers(
            self.show_city_boundaries,
            self.show_metro_boundaries,
            self.show_place_labels,
            self.cam.alt,
        );
        if self.boundary_loader.is_loading() {
            ctx.request_repaint_after(BOUNDARY_POLL_INTERVAL);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| self.globe_view(ui, ctx));
    }
}
