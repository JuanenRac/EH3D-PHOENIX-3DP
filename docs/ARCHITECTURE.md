# Architecture

EH3D-PHOENIX-3DP is a Cargo workspace: six small libraries with no external dependency, and the program that uses them (a library `eh3d_phoenix_3dp`, the command line `phoenix-3dp` and, with `--features gui`, a window). Every crate has `#![forbid(unsafe_code)]`. The picture of the README is drawn from `architecture.json`; this page says the same in words.

## The crates, from the bottom up

| Crate | What it holds | Depends on |
|---|---|---|
| `p3dp-core` | lengths in nanometres (`Nm`), the mesh (`Mesh`, `Point3`) and its checks (`validate`, `open_edges`, `volume_mm3`), the `Limits` of the readers, the settings-file reader (`kv`), a small JSON reader and writer, the seeded generator used by the tests, the `Secret` that prints `***`, and the machine types shared by everyone (`Dialect`, `TempLimits`, `Zone`) | nothing |
| `p3dp-formats` | readers of STL (text and binary), OBJ and basic 3MF; writers of binary STL, OBJ, the own 3MF package and the G-code file; the ZIP reader and writer, `inflate`, the CRC, a tag reader of XML | core |
| `p3dp-profiles` | the catalog of printers (`data/printers.tsv`, part of the source), the `PrinterProfile` and the profile file | core |
| `p3dp-gcode` | `Writer` (Marlin, Klipper, RepRapFirmware, Bambu), `check_program` (finds the lines of a program that go past the limits of a model) | core, formats |
| `p3dp-link` | the `PrinterDriver` trait, the normalized state and commands, the `HttpTransport` trait, the Moonraker and OctoPrint drivers, the retry rules, the in-memory simulators, and the design of Bambu LAN | core, formats, gcode |
| `p3dp-server-bridge` | the settings of the link to the server, the thin client of the draft routes of the printer plugin, a server in memory | core, link |
| `eh3d-phoenix-3dp` (this directory) | `cli` (the commands), `app_state` (what the window shows, with no window), `config`, `i18n` (eight languages), `project`, `view` (the orbit camera) and `gui` | all of the above |

## Rules that hold across the crates

* **A limit is a refusal.** `TempLimits::check` returns an error for a value above the limit; nothing in the program lowers a value to the limit. The same function is used by the G-code writer, by the drivers (through `driver::guard`) and by the checker of programs.
* **Units.** Lengths are whole nanometres inside the program, so adding and comparing never loses a digit; a number in millimetres is turned into `Nm` once, with a check that it is finite and not beyond a kilometre. Text is written back with the exact decimals.
* **Sizes before allocation.** Every reader checks the size it is promised (the triangles of an STL header, the entries of a ZIP package, the size it inflates to) against `Limits` before it reserves memory.
* **Doors.** Raw G-code is a command (`Command::Gcode`) that is refused unless the setting is on, is one line, and goes through the checker first. A file name or an identifier that goes into a path is checked (`safe_file_name`, `path_id`) and percent-encoded.
* **Secrets.** The token and the keys are `Secret` values; `Debug` and `Display` print `***`, and the settings are shown with `to_text(false)`.
* **Simulators, not networks.** The drivers speak to an `HttpTransport`. The tests give them `SimMoonraker`, `SimOctoPrint` or `SimBridgeServer`, which hold a printer or a server in memory and can fail on demand. No crate opens a socket in this version.

## The normalized state

Every driver answers `PrinterState`: the family, one of fifteen states (`offline`, `idle`, `preparing`, `printing`, `pausing`, `paused`, `finishing`, `finished`, `failed`, `cancelling`, `cancelled`, `error`, `emergency`, `updating`, `busy`), the raw text of the printer, the temperatures of the heaters, the job (file, progress, time) and the alerts. The tables that map what each protocol says onto the fifteen states are in `p3dp-link/src/state_map.rs` and are tested one row at a time.

## The window

`app_state::AppState` holds everything the four tabs show (Prepare, Preview, Device, Project): the printer chosen, the model loaded, the reports in the chosen language, the camera, and a simulated device for the Device tab. `gui.rs` only draws it and passes on what the person does, so the logic is tested without a screen. It is built with `--features gui` (egui) and has not been looked at on a screen yet.
