//! Schematic 3D viewer for `satellite-datetime` APIs.
//!
//! Not an ephemeris. Planet positions are layout, not SPICE.

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::input::ButtonInput;
use bevy::prelude::*;
use satellite_datetime::bodies::{self, EARTH, MARS, MOON};
use satellite_datetime::ccsds::{encode_cuc, CucConfig};
use satellite_datetime::earth::{
    format_rfc3339, from_posix_nanos, from_posix_seconds, si_nanos_since_unix_epoch,
    to_posix_seconds,
};
use satellite_datetime::lunar::ltc_status;
use satellite_datetime::tz::TimeZone;
use satellite_datetime::{parse_rfc3339, CivilUtc, Duration, Instant};

const EARTH_ORBIT: f32 = 7.0;
const MARS_ORBIT: f32 = 12.5;
const MOON_ORBIT: f32 = 1.6;

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
struct HudText;

#[derive(Component)]
struct EarthSpin;

#[derive(Component)]
struct MarsSpin;

#[derive(Component)]
struct MoonOrbit;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "satellite-datetime demo".into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<SimClock>()
        .add_systems(Startup, setup)
        .add_systems(Update, (tick_clock, handle_input, spin_bodies, update_hud))
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
        Transform::from_xyz(0.0, 9.0, 22.0).looking_at(Vec3::new(2.0, 0.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            intensity: 8_000_000.0,
            shadows_enabled: true,
            range: 40.0,
            ..default()
        },
        Transform::from_xyz(0.0, 4.0, 0.0),
    ));

    let sun = meshes.add(Sphere::new(1.2).mesh().ico(5).unwrap());
    commands.spawn((
        Mesh3d(sun),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.2),
            emissive: LinearRgba::rgb(4.0, 3.0, 0.4),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    let earth_mesh = meshes.add(Sphere::new(0.55).mesh().ico(4).unwrap());
    commands.spawn((
        Mesh3d(earth_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.15, 0.45, 0.9))),
        Transform::from_xyz(EARTH_ORBIT, 0.0, 0.0),
        EarthSpin,
    ));

    let moon_mesh = meshes.add(Sphere::new(0.16).mesh().ico(3).unwrap());
    commands.spawn((
        Mesh3d(moon_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.75, 0.75, 0.8))),
        Transform::from_xyz(EARTH_ORBIT + MOON_ORBIT, 0.0, 0.0),
        MoonOrbit,
    ));

    let mars_mesh = meshes.add(Sphere::new(0.4).mesh().ico(4).unwrap());
    commands.spawn((
        Mesh3d(mars_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.85, 0.35, 0.2))),
        Transform::from_xyz(MARS_ORBIT, 0.0, 0.0),
        MarsSpin,
    ));

    commands.spawn((
        Text::new("loading satellite-datetime…"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.95, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));
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
        let _ = bodies::PLANETS.len();
    }
}

fn update_hud(clock: Res<SimClock>, mut q: Query<&mut Text, With<HudText>>) {
    let Ok(mut text) = q.single_mut() else {
        return;
    };
    *text = Text::new(hud_string(&clock));
}

fn hud_string(clock: &SimClock) -> String {
    let t = clock.instant;
    let mut rfc = [0u8; 32];
    let utc_s = format_rfc3339(t, &mut rfc)
        .ok()
        .and_then(|n| std::str::from_utf8(&rfc[..n]).ok())
        .unwrap_or("(utc err)");

    let posix = to_posix_seconds(t).map(|s| s.to_string()).unwrap_or_else(|_| "err".into());
    let si = si_nanos_since_unix_epoch(t)
        .map(|n| format!("{:.3} s", n as f64 / 1e9))
        .unwrap_or_else(|_| "err".into());

    let gps = t
        .gps_week_sow()
        .map(|(w, s)| format!("week {w}  sow {s:.3}"))
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
                "{} {:04}-{:02}-{:02} {:02}:{:02}:{:02}  {}{:+}s",
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
        .unwrap_or_else(|_| "local err (maybe before 1960 UTC)".into());

    let pause = if clock.paused { "PAUSED" } else { "LIVE" };
    format!(
        "satellite-datetime  |  {pause}  speed {:.0}×\n\
         Space pause   1–4 speed   N now   G GPS epoch   L 2016 leap-15s   J J2000 UTC   Z zone\n\n\
         UTC     {utc_s}\n\
         local   {local}\n\
         POSIX   {posix}   (no leap seconds in the integer)\n\
         SI      {si} since Unix epoch (counts leaps)\n\
         TAI ns  {}\n\
         TT  − TAI = 32.184 s exactly   TCG−TT ≈ {:.3e} s\n\
         GPS     {gps}\n\
         Mars    MSD {:.6}   MTC {:02.0}h (mean solar, not UTC-like)\n\
         TCL ns  {}   LTC ns {}   [{}]\n\
         Moon surface proper vs TT ≈ {:.3} ms (mean rate, ×1 — HUD only)\n\
         CCSDS CUC 4+2  {cuc_s}\n\n\
         Schematic 3D: Earth spin = IAU sidereal hours; Mars spin = MTC. Not SPICE.",
        clock.speed,
        t.as_tai_nanos(),
        t.tcg_minus_tt().as_seconds_f64(),
        mars.msd,
        mars.mtc_hours,
        t.reading_tcl().as_nanos(),
        t.reading_ltc().as_nanos(),
        ltc_status(),
        t.lunar_mean_surface_proper().as_seconds_f64() * 1e3,
    )
}
