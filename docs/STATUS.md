# EH3D-PHOENIX-3DP: status

## Current state

EH3D-PHOENIX-3DP works on a computer against simulators: the command line reads, checks, converts and slices models, writes and checks G-code, and drives simulated Moonraker and OctoPrint printers and a double of the server; a docking window with five workspaces does the same with panels. The window has not been looked at on a screen by a person, the stand-alone Bambu link has no real TLS connector (only a seam and a printer made in memory), and nothing has run on a real printer.

## Recent work

The window became the docking frame of the suite with five workspaces for tasks, the filaments and the resins now come from the database of printing materials of the suite, a check before printing compares the temperatures and the flow with what the database says of the chosen filament, and the READMEs show drawings of the five workspaces made from the model of the window. The slicer can change tool between layers with a purge tower, iron the top and write the pressure advance, lay layers whose height follows the shape, draw in the first layer against the flared foot and hold every line to a limit of the flow (also the one the maker gives for the filament), and tell where the time goes; the Bambu link is made through the server with the certificate pinned, and a stand-alone driver reads the state and pauses, resumes and stops a print over a pinned secure channel, against a printer made in memory. The roadmap of the repository (`ROADMAP.md`) ties the work to the steps H0 to H9.

## Capabilities

The official state of each capability is in `REQUIREMENTS.md`. The register has 112 requirements: 98 implemented, 8 prototype, 6 planned and 0 validated. By progress: 98 done, 8 partial, 2 pending and 4 blocked. This page never states a state that `REQUIREMENTS.md` does not.

## Open questions

* Which printers are the first to be verified on hardware (a Bambu Lab A1 and a P1S are planned).
* Whether the stand-alone mode ever needs a TLS of its own: the Bambu link through the server has it, and a TLS of the program would need a crate behind a feature.
* Whether slicing may also run in the server.

## What it needs from other repositories

* EH3D-COMMON: the docking frame, the server client and the database of printing materials.
* EH3D-SERVER: the `printer3d` plugin whose routes the client speaks.
* A real printer, a real server and a person at them, for every proof that a simulator cannot give.

## Known technical debt and omitted tests

* 867 tests (`#[test]` lines of `src/`, `tests/` and `crates/`, the method of `tests/readme.rs`; nine of them keep this register true), all on a computer, with fixed seeds where chance is involved.
* No driver has spoken to a real printer or a real server; the simulators are written in this repository and prove the logic, not that a real printer answers the same way.
* The window has not been looked at on a screen by a person.
* The temperature limits of the catalog are the ones that the makers state and have not been compared with a printer.
