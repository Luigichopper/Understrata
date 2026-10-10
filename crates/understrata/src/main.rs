use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::app::AppExit;
use bevy::prelude::*;
use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser, Debug, Clone, Resource)]
#[command(name = "understrata", about = "Understrata — 3D Sandbox Adventure")]
pub struct Opts {
    /// Run without creating a graphical window (for tests, CI, software render)
    #[arg(long)]
    pub headless: bool,

    /// Dedicated server mode
    #[arg(long)]
    pub server: bool,

    /// World seed (number or string)
    #[arg(long)]
    pub seed: Option<String>,

    /// World size preset: small, medium, large
    #[arg(long, default_value = "small")]
    pub size: String,

    /// World save folder path
    #[arg(long)]
    pub world: Option<PathBuf>,

    /// Creative mode enabled
    #[arg(long)]
    pub creative: bool,

    /// Scripted bot command file (.ron)
    #[arg(long)]
    pub script: Option<PathBuf>,

    /// Terminate after N simulation ticks
    #[arg(long)]
    pub ticks: Option<u64>,

    /// Terminate after N render/game frames
    #[arg(long)]
    pub frames: Option<u64>,

    /// Terminate after S seconds
    #[arg(long)]
    pub seconds: Option<f32>,

    /// Fixed camera pos & angle: x,y,z,yaw,pitch
    #[arg(long)]
    pub camera: Option<String>,

    /// Capture screenshot at tick:path
    #[arg(long)]
    pub screenshot_at: Vec<String>,

    /// Chunk coords to dump: x,y,z
    #[arg(long)]
    pub dump_chunk: Option<String>,

    /// Dump chunk output path
    #[arg(long)]
    pub dump_out: Option<PathBuf>,

    /// Run benchmark scene (cave_dense, surface_forest, dig_stress, swarm_30)
    #[arg(long)]
    pub bench: Option<String>,

    /// Benchmark results CSV output path
    #[arg(long)]
    pub bench_out: Option<PathBuf>,

    /// Output structured JSON logs to path
    #[arg(long)]
    pub log_json: Option<PathBuf>,

    /// Log level filter: trace, debug, info, warn, error
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Fail test / exit non-zero if any warning is logged
    #[arg(long)]
    pub assert_no_warnings: bool,
}

#[derive(Resource)]
struct HarnessState {
    frame_count: u64,
    tick_count: u64,
    start_time: Instant,
    max_frames: Option<u64>,
    max_ticks: Option<u64>,
    max_seconds: Option<f32>,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    // Setup logging
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&opts.log_level));

    if let Some(json_path) = &opts.log_json {
        let file = std::fs::File::create(json_path)?;
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json().with_writer(file))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .init();
    }

    info!("Starting Understrata (headless={})...", opts.headless);

    let mut app = App::new();

    let harness = HarnessState {
        frame_count: 0,
        tick_count: 0,
        start_time: Instant::now(),
        max_frames: opts.frames,
        max_ticks: opts.ticks,
        max_seconds: opts.seconds,
    };
    app.insert_resource(harness);
    app.insert_resource(opts.clone());

    if opts.headless || opts.server {
        app.add_plugins(
            MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                Duration::from_secs_f64(1.0 / 60.0),
            )),
        );
    } else {
        app.add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::log::LogPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Understrata".into(),
                        resolution: (1280u32, 720u32).into(),
                        ..default()
                    }),
                    ..default()
                }),
        );
    }

    // Add Understrata subsystems
    app.add_plugins((
        us_game::UnderstrataGamePlugin,
        us_render::UnderstrataRenderPlugin,
        us_ui::UnderstrataUiPlugin,
        us_audio::UnderstrataAudioPlugin,
    ));

    app.add_systems(Update, harness_tick_system);

    app.run();

    Ok(())
}

fn harness_tick_system(mut harness: ResMut<HarnessState>, mut app_exit: MessageWriter<AppExit>) {
    harness.frame_count += 1;
    harness.tick_count += 1;

    let mut should_exit = false;

    if let Some(max_f) = harness.max_frames {
        if harness.frame_count >= max_f {
            info!("Reached requested {} frames. Exiting.", max_f);
            should_exit = true;
        }
    }

    if let Some(max_t) = harness.max_ticks {
        if harness.tick_count >= max_t {
            info!("Reached requested {} ticks. Exiting.", max_t);
            should_exit = true;
        }
    }

    if let Some(max_sec) = harness.max_seconds {
        if harness.start_time.elapsed().as_secs_f32() >= max_sec {
            info!("Reached requested {} seconds. Exiting.", max_sec);
            should_exit = true;
        }
    }

    if should_exit {
        app_exit.write(AppExit::Success);
    }
}
