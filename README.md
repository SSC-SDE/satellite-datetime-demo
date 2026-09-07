# satellite-datetime-demo

3D real-time visualizer for [`satellite-datetime`](https://crates.io/crates/satellite-datetime).
This is a **schematic** scene (not SPICE ephemerides). Clocks and labels are produced by the crate.

**Not flight-qualified.** Lunar TCL/LTC differences are microseconds — shown on the HUD, not as planet drift.

## Run

```bash
cargo run --release
```

Needs a GPU and a window (not a headless CI box).

## Controls

| Key | Action |
| --- | --- |
| Space | Pause / resume |
| `1` `2` `3` `4` | Speed 1× / 60× / 1 h per second / 1 day per second |
| `N` | Jump to OS clock (POSIX → `Instant`) |
| `G` | Jump to GPS epoch (1980-01-06) |
| `L` | Jump to 15 s before the 2016-12-31 leap second |
| `J` | Jump to 2000-01-01 12:00:00 UTC |
| `Z` | Cycle Earth zone (UTC, New York, London, Kolkata) |

## What you are looking at

- **Sun / Earth / Moon / Mars** — spheres. Earth and Mars spin from `satellite-datetime` sidereal / MTC hours.
- **HUD** — same `Instant` projected to TAI, TT, UTC (RFC 3339), POSIX vs SI elapsed, GPS week, Mars MSD/MTC, TCL/LTC, lunar surface offset, CCSDS CUC.

The library has no `now()`. This app injects time from the OS (or from a jump), then calls crate APIs every frame.

## License

MIT OR Apache-2.0. Depends on `satellite-datetime` (MIT OR Apache-2.0) and Bevy.
