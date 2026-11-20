# Architecture

EH3D-PHOENIX-3DP is a Cargo workspace: seven small libraries, none of which uses a crate from crates.io (the server bridge depends by path on the common client of the suite, `eh3d-serverclient` of the sibling repository EH3D-COMMON, which has no dependency either), and the program that uses them (a library `eh3d_phoenix_3dp`, the command line `phoenix-3dp` and, with `--features gui`, a window). Every crate has `#![forbid(unsafe_code)]`. The picture of the README is drawn from `architecture.json`; this page says the same in words.

## The crates, from the bottom up

| Crate | What it holds | Depends on |
|---|---|---|
| `p3dp-core` | lengths in nanometres (`Nm`), the mesh (`Mesh`, `Point3`) and its checks (`validate`, `open_edges`, `volume_mm3`), the `Limits` of the readers, the settings-file reader (`kv`), a small JSON reader and writer, the seeded generator used by the tests, the `Secret` that prints `***`, and the machine types shared by everyone (`Dialect`, `TempLimits`, `Zone`) | nothing |
| `p3dp-formats` | readers of STL (text and binary), OBJ and basic 3MF; `project3mf`, a read-only reader of the objects, plates and a few settings of the projects that other programs save as 3MF; writers of binary STL, OBJ, the own 3MF package and the G-code file; the ZIP reader and writer, `inflate`, the CRC, a tag reader of XML | core |
| `p3dp-profiles` | the catalog of printers (`data/printers.tsv`, part of the source), the `PrinterProfile` and the profile file | core |
| `p3dp-gcode` | `Writer` (Marlin, Klipper, RepRapFirmware, Bambu), `check_program` (finds the lines of a program that go past the limits of a model) | core, formats |
| `p3dp-link` | the `PrinterDriver` trait, the normalized state and commands, the `HttpTransport` trait and `TcpTransport` (plain HTTP/1.1 over a socket, with limits of time and size), the Moonraker and OctoPrint drivers, the retry rules, the in-memory simulators, and the design of Bambu LAN | core, formats, gcode |
| `p3dp-server-bridge` | the settings of the link to the server, the client of the routes of the `printer3d` plugin (`/api/v1/work/...`) over `eh3d-serverclient`, a server in memory built on the one of the suite | core, gcode, eh3d-serverclient |
| `p3dp-slicer` | the cut of a mesh by flat layers, perimeters, rectilinear infill with solid skins, the plan of paths by layer (the preview as data), G-code through the writer of the program | core, formats, profiles, gcode |
| `eh3d-phoenix-3dp` (this directory) | `cli` (the commands), `app_state` (what the window shows, with no window), `config`, `i18n` (eight languages), `project`, `view` (the orbit camera) and `gui` | all of the above |

## Rules that hold across the crates

* **A limit is a refusal.** `TempLimits::check` returns an error for a value above the limit; nothing in the program lowers a value to the limit. The same function is used by the G-code writer, by the drivers (through `driver::guard`) and by the checker of programs.
* **Units.** Lengths are whole nanometres inside the program, so adding and comparing never loses a digit; a number in millimetres is turned into `Nm` once, with a check that it is finite and not beyond a kilometre. Text is written back with the exact decimals.
* **Sizes before allocation.** Every reader checks the size it is promised (the triangles of an STL header, the entries of a ZIP package, the size it inflates to) against `Limits` before it reserves memory.
* **Doors.** Raw G-code is a command (`Command::Gcode`) that is refused unless the setting is on, is one line, and goes through the checker first. A file name or an identifier that goes into a path is checked (`safe_file_name`, `path_id`) and percent-encoded.
* **Secrets.** The token and the keys are `Secret` values; `Debug` and `Display` print `***`, and the settings are shown with `to_text(false)`.
* **A network only where an address is given.** The drivers speak to an `HttpTransport`. The tests give them `SimMoonraker` or `SimOctoPrint` (a printer in memory that can fail on demand) or `TcpTransport` against a socket of `127.0.0.1` that the test itself opens; the server client is given `SimPrinterServer`, built on the server in memory of the suite. Nothing opens a connection unless an address is set; there is no TLS (an `https://` address is refused with the reason) and a redirect is not followed.

## The slicer

`p3dp-slicer` cuts the mesh at the middle of every layer (a vertex on the plane counts as above it, so the cuts of neighbouring triangles meet exactly), joins the segments into loops, offsets the loops inwards for the perimeters (cutting apart the loops that an offset folds over itself, and leaving out what is thinner than the perimeters, with a warning or a refusal as `thin_regions` says), fills the rest with straight lines, and makes the infill solid where the layers above or below do not cover it. Every value is in the settings file (`SliceConfig`, with a range for each key); a temperature over the limit of the model, a model that does not fit the bed, a mesh that is not closed or inside out and a setting out of its range are errors, never adjusted. The G-code is written through the `Writer` of the program, which refuses a coordinate outside the bed, so what the plan says and what the file says cannot differ.

## The normalized state

Every driver answers `PrinterState`: the family, one of fifteen states (`offline`, `idle`, `preparing`, `printing`, `pausing`, `paused`, `finishing`, `finished`, `failed`, `cancelling`, `cancelled`, `error`, `emergency`, `updating`, `busy`), the raw text of the printer, the temperatures of the heaters, the job (file, progress, time) and the alerts. The tables that map what each protocol says onto the fifteen states are in `p3dp-link/src/state_map.rs` and are tested one row at a time.

## The window

`app_state::AppState` holds everything the four tabs show (Prepare, Preview, Device, Project): the printer chosen, the model loaded, the reports in the chosen language, the camera, and a simulated device for the Device tab. `gui.rs` only draws it and passes on what the person does, so the logic is tested without a screen. It is built with `--features gui` (egui) and has not been looked at on a screen yet.
