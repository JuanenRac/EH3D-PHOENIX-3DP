# EH3D-PHOENIX-3DP: scope

Status: **functional**. EH3D-PHOENIX-3DP is a workspace of eight libraries, a command line (`phoenix-3dp`) and a window (built with `--features gui`) to prepare 3D prints: it reads models, holds a catalog of printer profiles, slices a closed model into flat layers, writes safe G-code, and talks to printers and to the EH3D server. It has run on a computer against simulators only. This page says what it is for, what it will not do and where it sits among the other programs of the suite.

## Purpose

The 3D-printing program of the PHOENIX family: it reads and converts STL, OBJ and 3MF models, checks a printer and a filament or a resin against each other, slices a model with a small slicer, writes G-code for Marlin, Klipper, RepRapFirmware and Bambu with temperature limits that refuse a value instead of cutting it down, and sends, follows and stops prints through the EH3D server or straight to a printer. The server is the centre; the stand-alone mode is optional.

## What it does

* Reads STL (ASCII and binary), OBJ and 3MF models within limits, checks the mesh (open, inverted, degenerate) and converts between the formats; reads the metadata of binary G-code, plain G-code and the projects of other programs without writing them back.
* Shows the catalog of printers of the suite, each figure with the page of its maker, and brings the settings of a 3MF project or of a JSON or INI profile of the person, telling every key it applied, did not apply, did not use or did not read. It sends nothing to a printer.
* Holds a catalog of 120 printer models of both technologies with the limits their makers publish (none verified), and the filaments and resins of the database of printing materials, checking a printer, a nozzle and a material against each other and listing the reasons instead of lowering a temperature.
* Slices a closed model into layers with perimeters, rectilinear, grid or gyroid infill, solid skins, bridges, plain supports, seams, a skirt, a brim or a raft, a vase mode, slower short layers, retraction that follows the travel, and an estimate of the time with acceleration.
* Writes G-code for four dialects and checks any program against the limits of the printer; what the writer refuses the checker also finds.
* Drives Moonraker and OctoPrint printers through a plain HTTP transport, and the local link of Bambu as bytes against a broker in memory, always keeping to the limits of the printer; raw G-code is off unless switched on.
* Talks to the EH3D server through its client: the queue never starts a job without a question, the emergency stop latches the machine, and the token is never shown.
* Shows the work in a docking window of panels and four workspaces: the bed in 3D, the parts, arrangement, the plan by layers with a slider, the queue and the state of the machine, in eight languages.

## What it will not do

* It does not control a printer by itself and is not a safety system: what it produces must be checked by a person before anything is run.
* It never lowers a temperature or a limit in silence: a value over a limit is refused with the key and the limit.
* It does not open a TLS connection: an `https://` address is refused, and the Bambu local link waits for a decision on TLS.
* It does not paint colours on a surface, make tree supports or interface layers, or open a TLS connection of its own to a real printer yet, and it has no driver for resin printers.
* It does not claim that anything was proven on a real printer: the evidence of every capability is `simulator` or `host-test`.

## Where it sits

* **It reads:** models (STL, OBJ, 3MF), the metadata of G-code and of the projects of other programs, packages of resin layers, its settings and profile files, the database of printing materials, and the answers of printers and of the server.
* **It writes:** converted models, G-code programs, projects and profiles that the person asks for, and its settings and arrangement; a file that exists is not replaced by accident. To a printer or to the server it writes only orders that keep to the limits.
* **It is read by:** the EH3D server receives its jobs; no other program reads it.
* **It depends on:** EH3D-COMMON (the docking frame, the server client and the database of printing materials) as a sibling folder; its own eight crates; and no outside crate.

## What it never decides

Whether a print is safe, and the temperature limits, the interlocks and the emergency stop of a printer, belong to the printer, its firmware and the person at it. The limits of the catalog are the ones the makers state and have not been compared with a printer; a wrong limit is a bug to report, not a decision of this program. Nothing here replaces the emergency stop of the printer.

The requirements of the program, with the state of each and the test that shows it, are in `REQUIREMENTS.md`; the state in a paragraph is in `STATUS.md`.
