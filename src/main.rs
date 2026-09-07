//! Schematic 3D viewer for `satellite-datetime` APIs.
//!
//! Not an ephemeris. Planet positions are layout, not SPICE.
//! Left column: crate projections of one Instant. Right: schematic bodies.

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::render::camera::{ClearColorConfig, Viewport};
use bevy::ui::IsDefaultUiCamera;
use bevy::window::PrimaryWindow;
use satellite_datetime::ccsds::{encode_cuc, CucConfig};
use satellite_datetime::earth::{
    format_rfc3339, from_posix_nanos, si_nanos_since_unix_epoch, to_posix_seconds,
};
use satellite_datetime::lunar::ltc_status;
use satellite_datetime::tz::TimeZone;
use satellite_datetime::{parse_rfc3339, CivilUtc, Duration, Instant};
use satellite_datetime::bodies::{EARTH, MOON};

const EARTH_ORBIT: f32 = 7.0;
const MARS_ORBIT: f32 = 12.5;
const MOON_ORBIT: f32 = 1.6;
/// Logical pixels reserved for the data column (physical size uses window scale).
const DATA_PANEL_PX: f32 = 428.0;

const INK: Color = Color::srgb(0.92, 0.93, 0.95);
const MUTED: Color = Color::srgb(0.62, 0.66, 0.72);
const ACCENT: Color = Color::srgb(0.86, 0.72, 0.38);
const LIVE: Color = Color::srgb(0.42, 0.86, 0.58);
const PAUSED: Color = Color::srgb(0.95, 0.72, 0.32);
const PANEL_BG: Color = Color::srgb(0.07, 0.08, 0.10);
const CARD_BG: Color = Color::srgb(0.13, 0.15, 0.18);
const RULE: Color = Color::srgb(0.22, 0.24, 0.28);

#[derive(Resource)]
struct SimClock {
    instant: Instant,
    speed: f64,
    paused: bool,
    zone: TimeZone,
}

impl Default for SimClock {
    fn default() -> Self {
        Self {
            instant: wall_instant(),
            speed: 1.0,
            paused: false,
            zone: TimeZone::Utc,
        }
    }
}

#[derive(Component)]
struct SceneCamera;

#[derive(Component)]
struct EarthSpin;

#[derive(Component)]
struct MarsSpin;

#[derive(Component)]
struct MoonOrbit;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HudSlot {
    Status,
    Utc,
    Local,
    Posix,
    Si,
    Tai,
    Relativity,
    Gps,
    Mars,
    Lunar,
    Cuc,
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.04, 0.045, 0.055)))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.55, 0.6, 0.7),
            brightness: 120.0,
            ..default()
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "satellite-datetime demo".into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<SimClock>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                tick_clock,
                handle_input,
                spin_bodies,
                fit_scene_viewport,
                update_hud,
            ),
        )
        .run();
}

fn wall_instant() -> Instant {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0);
    from_posix_nanos(ns).unwrap_or(Instant::TAI_EPOCH)
}

fn tick_clock(time: Res<Time>, mut clock: ResMut<SimClock>) {
    if clock.paused {
        return;
    }
    let dt = time.delta_secs_f64() * clock.speed;
    if dt <= 0.0 {
        return;
    }
    if let Ok(step) = Duration::from_seconds_f64(dt) {
        if let Ok(next) = clock.instant.checked_add(step) {
            clock.instant = next;
        }
    }
}

fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut clock: ResMut<SimClock>) {
    if keys.just_pressed(KeyCode::Space) {
        clock.paused = !clock.paused;
    }
    if keys.just_pressed(KeyCode::Digit1) {
        clock.speed = 1.0;
        clock.paused = false;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        clock.speed = 60.0;
        clock.paused = false;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        clock.speed = 3_600.0;
        clock.paused = false;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        clock.speed = 86_400.0;
        clock.paused = false;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        clock.instant = wall_instant();
    }
    if keys.just_pressed(KeyCode::KeyG) {
        if let Ok(c) = CivilUtc::new(1980, 1, 6, 0, 0, 0, 0) {
            if let Ok(t) = c.to_instant() {
                clock.instant = t;
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyL) {
        if let Ok(t) = parse_rfc3339("2016-12-31T23:59:45Z") {
            clock.instant = t;
            clock.speed = 1.0;
            clock.paused = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyJ) {
        if let Ok(c) = CivilUtc::new(2000, 1, 1, 12, 0, 0, 0) {
            if let Ok(t) = c.to_instant() {
                clock.instant = t;
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        clock.zone = next_zone(clock.zone);
    }
}

fn next_zone(z: TimeZone) -> TimeZone {
    match z {
        TimeZone::Utc => TimeZone::AmericaNewYork,
        TimeZone::AmericaNewYork => TimeZone::EuropeLondon,
        TimeZone::EuropeLondon => TimeZone::AsiaKolkata,
        _ => TimeZone::Utc,
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 0,
            ..default()
        },
        Transform::from_xyz(4.5, 7.2, 18.0).looking_at(Vec3::new(7.5, 0.0, 0.0), Vec3::Y),
        SceneCamera,
    ));

    let ui_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            IsDefaultUiCamera,
        ))
        .id();

    commands.spawn((
        PointLight {
            intensity: 6_500_000.0,
            shadows_enabled: true,
            range: 48.0,
            color: Color::srgb(1.0, 0.95, 0.8),
            ..default()
        },
        Transform::from_xyz(0.0, 0.4, 0.0),
    ));

    spawn_orbit_ring(
        &mut commands,
        &mut meshes,
        &mut materials,
        EARTH_ORBIT,
        Color::srgba(0.35, 0.55, 0.9, 0.35),
    );
    spawn_orbit_ring(
        &mut commands,
        &mut meshes,
        &mut materials,
        MARS_ORBIT,
        Color::srgba(0.85, 0.4, 0.25, 0.3),
    );

    let sun = meshes.add(Sphere::new(1.15).mesh().ico(5).unwrap());
    commands.spawn((
        Mesh3d(sun),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.88, 0.35),
            emissive: LinearRgba::rgb(5.0, 3.6, 0.5),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    let earth_mesh = meshes.add(Sphere::new(0.55).mesh().ico(4).unwrap());
    commands.spawn((
        Mesh3d(earth_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.18, 0.48, 0.92))),
        Transform::from_xyz(EARTH_ORBIT, 0.0, 0.0),
        EarthSpin,
    ));

    let moon_mesh = meshes.add(Sphere::new(0.16).mesh().ico(3).unwrap());
    commands.spawn((
        Mesh3d(moon_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.78, 0.78, 0.82))),
        Transform::from_xyz(EARTH_ORBIT + MOON_ORBIT, 0.0, 0.0),
        MoonOrbit,
    ));

    let mars_mesh = meshes.add(Sphere::new(0.4).mesh().ico(4).unwrap());
    commands.spawn((
        Mesh3d(mars_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.86, 0.38, 0.22))),
        Transform::from_xyz(MARS_ORBIT, 0.0, 0.0),
        MarsSpin,
    ));

    spawn_ui(&mut commands, ui_camera);
}

fn spawn_orbit_ring(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    radius: f32,
    color: Color,
) {
    let mesh = meshes.add(Annulus::new(radius - 0.035, radius + 0.035));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        })),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
}

fn spawn_ui(commands: &mut Commands, ui_camera: Entity) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            UiTargetCamera(ui_camera),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(DATA_PANEL_PX),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(10.0),
                    overflow: Overflow::scroll_y(),
                    border: UiRect::right(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                BorderColor(RULE),
            ))
            .with_children(|col| {
                spawn_header(col);
                spawn_card(
                    col,
                    "EARTH CIVIL",
                    "Human calendars. One Instant, two clocks: UTC and a zoned civil time.",
                    &[
                        ("UTC RFC 3339", HudSlot::Utc),
                        ("Local (Z cycles zone)", HudSlot::Local),
                    ],
                );
                spawn_card(
                    col,
                    "COMPUTER TIME",
                    "POSIX integer seconds skip leap seconds. SI elapsed since Unix includes them.",
                    &[
                        ("POSIX seconds", HudSlot::Posix),
                        ("SI since Unix epoch", HudSlot::Si),
                    ],
                );
                spawn_card(
                    col,
                    "ATOMIC / RELATIVITY",
                    "Internal model is TAI nanoseconds since 1958-01-01. TT and TCG are projections.",
                    &[
                        ("TAI", HudSlot::Tai),
                        ("TT and TCG", HudSlot::Relativity),
                    ],
                );
                spawn_card(
                    col,
                    "NAVIGATION",
                    "GNSS week from the same Instant (GPS epoch 1980-01-06).",
                    &[("GPS week / SOW", HudSlot::Gps)],
                );
                spawn_card(
                    col,
                    "OTHER WORLDS",
                    "Mars is mean solar (MSD/MTC), not a UTC clone. Lunar TCL/LTC is linear IAU 2024.",
                    &[
                        ("Mars", HudSlot::Mars),
                        ("Moon", HudSlot::Lunar),
                    ],
                );
                spawn_card(
                    col,
                    "ON THE WIRE",
                    "CCSDS unsegmented time code, 4 coarse + 2 fine octets.",
                    &[("CUC 4+2", HudSlot::Cuc)],
                );
                spawn_help(col);
            });

            root.spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(16.0)),
                    ..default()
                },
            ))
            .with_children(|scene| {
                scene
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.55)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|box_| {
                        box_.spawn((
                            Text::new("SCHEMATIC BODIES"),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(ACCENT),
                        ));
                        box_.spawn((
                            Text::new(
                                "Layout only -- not SPICE. Earth spin uses IAU sidereal hours; Mars spin uses MTC.",
                            ),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(INK),
                        ));
                    });

                scene
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(14.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.55)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|legend| {
                        spawn_legend_chip(legend, Color::srgb(1.0, 0.85, 0.25), "Sun");
                        spawn_legend_chip(legend, Color::srgb(0.25, 0.5, 0.95), "Earth");
                        spawn_legend_chip(legend, Color::srgb(0.8, 0.8, 0.85), "Moon");
                        spawn_legend_chip(legend, Color::srgb(0.86, 0.38, 0.22), "Mars");
                    });
            });
        });
}

fn spawn_header(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(CARD_BG),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .with_children(|h| {
            h.spawn((
                Text::new("satellite-datetime"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(INK),
            ));
            h.spawn((
                Text::new("One Instant, many projections"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(MUTED),
            ));
            h.spawn((
                Text::new("LIVE"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(LIVE),
                HudSlot::Status,
            ));
        });
}

fn spawn_card(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    blurb: &str,
    rows: &[(&str, HudSlot)],
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(CARD_BG),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(title),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(ACCENT),
            ));
            card.spawn((
                Text::new(blurb),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(MUTED),
            ));
            for (label, slot) in rows {
                card.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        padding: UiRect::top(Val::Px(4.0)),
                        border: UiRect::top(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor(RULE),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(*label),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(MUTED),
                    ));
                    row.spawn((
                        Text::new("..."),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(INK),
                        *slot,
                    ));
                });
            }
        });
}

fn spawn_help(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(CARD_BG),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .with_children(|h| {
            h.spawn((
                Text::new("CONTROLS"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(ACCENT),
            ));
            for line in [
                "Space  pause / resume",
                "1-4    1x / 60x / 1 h per s / 1 d per s",
                "N      OS clock (POSIX -> Instant)",
                "G      GPS epoch    L  2016 leap -15s",
                "J      2000-01-01 12:00 UTC",
                "Z      cycle Earth zone",
            ] {
                h.spawn((
                    Text::new(line),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(MUTED),
                ));
            }
        });
}

fn spawn_legend_chip(parent: &mut ChildSpawnerCommands, color: Color, name: &'static str) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|chip| {
            chip.spawn((
                Node {
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(color),
                BorderRadius::all(Val::Px(5.0)),
            ));
            chip.spawn((
                Text::new(name),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(INK),
            ));
        });
}

fn fit_scene_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<SceneCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let scale = window.resolution.scale_factor();
    let inset = (DATA_PANEL_PX * scale).round() as u32;
    let width = window.physical_width().saturating_sub(inset).max(1);
    let height = window.physical_height().max(1);
    camera.viewport = Some(Viewport {
        physical_position: UVec2::new(inset, 0),
        physical_size: UVec2::new(width, height),
        ..default()
    });
}

fn spin_bodies(
    clock: Res<SimClock>,
    mut earth: Query<&mut Transform, (With<EarthSpin>, Without<MarsSpin>, Without<MoonOrbit>)>,
    mut mars: Query<&mut Transform, (With<MarsSpin>, Without<EarthSpin>, Without<MoonOrbit>)>,
    mut moon: Query<&mut Transform, (With<MoonOrbit>, Without<EarthSpin>, Without<MarsSpin>)>,
) {
    let t = clock.instant;
    if let Ok(mut tf) = earth.single_mut() {
        let hours = EARTH.sidereal_hours(t) as f32;
        tf.rotation = Quat::from_rotation_y(hours / 24.0 * std::f32::consts::TAU);
        tf.translation = Vec3::new(EARTH_ORBIT, 0.0, 0.0);
    }
    if let Ok(mut tf) = mars.single_mut() {
        let hours = t.mars_time().mtc_hours as f32;
        tf.rotation = Quat::from_rotation_y(hours / 24.0 * std::f32::consts::TAU);
        tf.translation = Vec3::new(MARS_ORBIT, 0.0, 0.0);
    }
    if let Ok(mut tf) = moon.single_mut() {
        let hours = MOON.sidereal_hours(t) as f32;
        let angle = hours / 24.0 * std::f32::consts::TAU;
        tf.translation = Vec3::new(
            EARTH_ORBIT + MOON_ORBIT * angle.cos(),
            0.15,
            MOON_ORBIT * angle.sin(),
        );
    }
}

fn update_hud(clock: Res<SimClock>, mut q: Query<(&HudSlot, &mut Text, Option<&mut TextColor>)>) {
    let snap = HudSnapshot::from_clock(&clock);
    for (slot, mut text, color) in &mut q {
        *text = Text::new(snap.text(*slot));
        if *slot == HudSlot::Status {
            if let Some(mut c) = color {
                c.0 = if clock.paused { PAUSED } else { LIVE };
            }
        }
    }
}

struct HudSnapshot {
    status: String,
    utc: String,
    local: String,
    posix: String,
    si: String,
    tai: String,
    relativity: String,
    gps: String,
    mars: String,
    lunar: String,
    cuc: String,
}

impl HudSnapshot {
    fn from_clock(clock: &SimClock) -> Self {
        let t = clock.instant;
        let mut rfc = [0u8; 32];
        let utc = format_rfc3339(t, &mut rfc)
            .ok()
            .and_then(|n| std::str::from_utf8(&rfc[..n]).ok())
            .unwrap_or("(utc err)")
            .to_string();

        let posix = to_posix_seconds(t)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "err".into());
        let si = si_nanos_since_unix_epoch(t)
            .map(|n| format!("{:.3} s", n as f64 / 1e9))
            .unwrap_or_else(|_| "err".into());

        let gps = t
            .gps_week_sow()
            .map(|(w, s)| format!("week {w}   SOW {s:.3}"))
            .unwrap_or_else(|_| "err".into());

        let mars = t.mars_time();
        let mut cuc = [0u8; 8];
        let cuc_s = encode_cuc(t, CucConfig::C4_F2, &mut cuc)
            .map(|n| {
                cuc[..n]
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|_| "err".into());

        let local = clock
            .zone
            .to_local(t)
            .map(|(c, off)| {
                format!(
                    "{}  {:04}-{:02}-{:02}  {:02}:{:02}:{:02}  {} {:+}s",
                    clock.zone.iana_name(),
                    c.year,
                    c.month,
                    c.day,
                    c.hour,
                    c.minute,
                    c.second,
                    off.abbr,
                    off.seconds_east
                )
            })
            .unwrap_or_else(|_| "unavailable (before 1960 UTC)".into());

        let run = if clock.paused { "PAUSED" } else { "LIVE" };
        let speed = speed_label(clock.speed);
        let status = format!("{run}   {speed}   zone {}", clock.zone.iana_name());

        let tcg = t.tcg_minus_tt().as_seconds_f64();
        let lunar_ms = t.lunar_mean_surface_proper().as_seconds_f64() * 1e3;
        let ltc = ascii_status(ltc_status());

        Self {
            status,
            utc,
            local,
            posix: format!("{posix}  (leap seconds omitted)"),
            si: format!("{si}  (leaps counted)"),
            tai: format!("{} ns since 1958-01-01", t.as_tai_nanos()),
            relativity: format!("TT - TAI = 32.184 s exactly\nTCG - TT ~ {tcg:.3e} s"),
            gps,
            mars: format!(
                "MSD {:.6}\nMTC {:05.2} h  (mean solar)",
                mars.msd, mars.mtc_hours
            ),
            lunar: format!(
                "TCL {} ns\nLTC {} ns\n{}\nsurface proper vs TT ~ {:.3} ms (HUD only)",
                t.reading_tcl().as_nanos(),
                t.reading_ltc().as_nanos(),
                ltc,
                lunar_ms
            ),
            cuc: cuc_s,
        }
    }

    fn text(&self, slot: HudSlot) -> String {
        match slot {
            HudSlot::Status => self.status.clone(),
            HudSlot::Utc => self.utc.clone(),
            HudSlot::Local => self.local.clone(),
            HudSlot::Posix => self.posix.clone(),
            HudSlot::Si => self.si.clone(),
            HudSlot::Tai => self.tai.clone(),
            HudSlot::Relativity => self.relativity.clone(),
            HudSlot::Gps => self.gps.clone(),
            HudSlot::Mars => self.mars.clone(),
            HudSlot::Lunar => self.lunar.clone(),
            HudSlot::Cuc => self.cuc.clone(),
        }
    }
}

fn speed_label(speed: f64) -> &'static str {
    if (speed - 1.0).abs() < f64::EPSILON {
        "1x"
    } else if (speed - 60.0).abs() < f64::EPSILON {
        "60x"
    } else if (speed - 3_600.0).abs() < f64::EPSILON {
        "1 hour / s"
    } else if (speed - 86_400.0).abs() < f64::EPSILON {
        "1 day / s"
    } else {
        "custom"
    }
}

fn ascii_status(s: &str) -> String {
    s.replace('\u{2013}', "-")
        .replace('\u{2014}', "-")
        .replace('\u{2212}', "-")
        .replace('\u{00d7}', "x")
        .replace('\u{2248}', "~")
}
