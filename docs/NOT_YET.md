# What is not done yet

This page lists what the program does not do, and what each missing part needs. Nothing on it is hidden in the README: the table "Project status" says the same with lights.

## A small slicer

The program turns a closed model into flat layers, perimeters, a rectilinear, grid or gyroid infill with solid skins, bridges, plain supports, seams, variable width of the perimeters and G-code, and shows the plan by layers as data. It has no tree supports, interface layers or painting of supports, no ironing, pressure advance, brim or skirt, adaptive layer height or multi-material; a bridge is laid across the gap but is not tuned (no flow test, no extra anchor); the supports are lines at a pitch, with no interface; the time it estimates has an acceleration and a speed at the corners from the settings, not the planner of the printer; and no print made from its G-code has been run on a printer. A model that is not closed, that faces inwards or that does not fit is refused, not repaired or scaled.

## Nothing has run on a printer

* No driver has spoken to a real printer. The Moonraker and OctoPrint drivers follow the public description of the protocols and have been tried only against the simulators of this repository, which are written by us and prove the logic, not that a real server answers the same way.
* Every profile of the catalog says `verified = false`. A profile becomes verified when someone has printed with it on that model and says so (an issue with the form "Hardware validation result"). A Bambu Lab A1 and a P1S are the first printers planned for that.
* The temperature limits are the ones that the makers state in their data sheets. They have not been compared with a printer, and a wrong limit in the catalog is a bug to report.

## Network

The drivers talk to an `HttpTransport`; `TcpTransport` is a plain HTTP/1.1 one written with the standard library, tried only against a socket of the same computer. It has no TLS (an `https://` address is refused), follows no redirect and keeps no connection open. The WebSocket push of Moonraker and the push of OctoPrint are step H7. The retry rules (a refused connection is tried again for any request; a time-out only for requests that can be sent twice; never a POST) are tested against the simulators.

## Bambu Lab

`p3dp-link/src/bambu.rs` holds what can be written without a printer: the table of states, the names of the topics, the messages that ask for the whole state and that pause, resume or stop. The driver itself answers `Experimental` to everything. It needs, before it connects: a client of MQTT over TLS with the certificate of the printer checked, the upload by FTPS, and the access code of the printer in a store that is encrypted at rest. The cloud of Bambu is not part of the design and, if it is ever added, will be off by default.

## Models

The projects that other programs save as 3MF are read for their metadata only (which program made them, the objects, the plates and a few settings), from the layout of their files as it looks in a package, and nothing is written back; the settings that are not in that short list are not read. The 3MF reader of the mesh takes the mesh objects of the core specification, with units and the transform of the build items. It refuses components, objects in other files (the production extension), and it ignores colours, materials, plates and the settings that other programs store in a package. Writing a 3MF that carries the G-code for a Bambu printer is step H6.

## Resin printers

The catalog has resin (mSLA) printers, with their volume and how they connect, but they take layers and not G-code: the writer refuses them and the drivers do not exist yet. The open packages of layers (`.sl1`) are read by their headers, with an estimate of the exposure, and nothing else: no layer image is decoded, no file is written, and the closed formats of the makers are refused. Binary G-code is read the same way, by its headers, following the public description of the format and tried only on files that the tests make by it.

## The window

It has four tabs and its logic is tested, but it has not been built or looked at on a screen. The model of the 3D viewer is the crate `p3dp-viewer` (camera, picking, selection box, range of layers, lines of a plan); the drawing with wgpu is behind its feature `gui`, never built, and the window does not use it yet: today the preview is lines. The texts of the new panels and controls are in the eight languages, but the panels themselves are not in the window. The names of the fifteen printer states are translated in the eight languages.

## The server

The client speaks the routes of the `printer3d` plugin of the server through the common client of the suite, but has only met a double of them in memory: it has never talked to a running `eh3d-server`. The name `phoenix-3dp` is not among the applications the server knows by default, so a login needs it in `auth_extra_apps` (or a token of a program).
