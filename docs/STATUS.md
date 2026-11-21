# EH3D-PHOENIX-3DP: status

## Current state

EH3D-PHOENIX-3DP works on a computer against simulators: the command line reads, checks, converts and slices models, writes and checks G-code, and drives simulated Moonraker and OctoPrint printers and a double of the server; a docking window with five workspaces does the same with panels. The window has not been looked at on a screen by a person, the Bambu local link has no TLS, and nothing has run on a real printer.

## Recent work

The window became the docking frame of the suite with four workspaces for tasks, the filaments and the resins now come from the database of printing materials of the suite, a check before printing compares the temperatures and the flow with what the database says of the chosen filament, and the READMEs show drawings of the five workspaces made from the model of the window. The roadmap of the repository (`ROADMAP.md`) ties the work to the steps H0 to H9.

## Capabilities

The official state of each capability is in `REQUIREMENTS.md`. The register has 99 requirements: 88 implemented, 5 prototype, 6 planned and 0 validated. By progress: 88 done, 5 partial, 3 pending and 3 blocked. This page never states a state that `REQUIREMENTS.md` does not.

## Open questions

* Which printers are the first to be verified on hardware (a Bambu Lab A1 and a P1S are planned).
* How the Bambu local link gets TLS: through the server, behind a feature, or with the TLS of the system.
* Whether slicing may also run in the server.

## What it needs from other repositories

* EH3D-COMMON: the docking frame, the server client and the database of printing materials.
* EH3D-SERVER: the `printer3d` plugin whose routes the client speaks.
* A real printer, a real server and a person at them, for every proof that a simulator cannot give.

## Known technical debt and omitted tests

* 659 tests (`#[test]` lines of `src/`, `tests/` and `crates/`, the method of `tests/readme.rs`; nine of them keep this register true), all on a computer, with fixed seeds where chance is involved.
* No driver has spoken to a real printer or a real server; the simulators are written in this repository and prove the logic, not that a real printer answers the same way.
* The window has not been looked at on a screen by a person.
* The temperature limits of the catalog are the ones that the makers state and have not been compared with a printer.
