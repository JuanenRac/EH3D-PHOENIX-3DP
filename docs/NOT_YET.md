# What is not done yet

This page lists what the program does not do, and what each missing part needs. Nothing on it is hidden in the README: the table "Project status" says the same with lights.

## A small slicer

The program turns a closed model into flat layers, perimeters, a rectilinear, grid or gyroid infill with solid skins, bridges, plain supports, seams, variable width of the perimeters and G-code, and shows the plan by layers as data. It has no tree supports, interface layers or painting of supports, no ironing, pressure advance, adaptive layer height or multi-material (it does lay a skirt, a brim or a raft, slow a short layer, shorten the retraction of a short travel and print a vase as one wall of separate layers, not as a spiral); a bridge is laid across the gap but is not tuned (no flow test, no extra anchor); the supports are lines at a pitch, with no interface; the time it estimates has an acceleration and a speed at the corners from the settings, not the planner of the printer; and no print made from its G-code has been run on a printer. A model that is not closed, that faces inwards or that does not fit is refused, not repaired or scaled.

## Nothing has run on a printer

* No driver has spoken to a real printer. The Moonraker and OctoPrint drivers follow the public description of the protocols and have been tried only against the simulators of this repository, which are written by us and prove the logic, not that a real server answers the same way.
* Every profile of the catalog says `verified = false`. A profile becomes verified when someone has printed with it on that model and says so (an issue with the form "Hardware validation result"). A Bambu Lab A1 and a P1S are the first printers planned for that.
* The temperature limits are the ones that the makers state in their data sheets. They have not been compared with a printer, and a wrong limit in the catalog is a bug to report.

## Network

The drivers talk to an `HttpTransport`; `TcpTransport` is a plain HTTP/1.1 one written with the standard library, tried only against a socket of the same computer. It has no TLS (an `https://` address is refused), follows no redirect and keeps no connection open. The WebSocket push of Moonraker and the push of OctoPrint are step H7. The retry rules (a refused connection is tried again for any request; a time-out only for requests that can be sent twice; never a POST) are tested against the simulators.

## Bambu Lab

`p3dp-link/src/bambu.rs` holds what can be written and tested without a printer: the table of states, the names of the topics, the messages that ask for the whole state and that pause, resume or stop, and the settings of the local link (`LanSettings`), which are refused without the fingerprint of the certificate of the printer. Around it are three modules with the protocol as bytes and text, tried with a broker made in memory and with the published test vectors: `mqtt` (the packets of MQTT 3.1.1 that the link uses, quality 0 only), `ftps` (the commands and replies of the upload, with the check that the data connection goes to the address of the control connection) and `pin` (SHA-256 and the comparison of the fingerprint of a certificate). The driver itself still answers `Experimental` to everything, because nothing opens the TLS connection: the standard library has no TLS and this repository takes no crates from outside.

Pending, with the ways to close it:

* **A. Through the EH3D server (the one this program recommends).** The server has the experimental Bambu LAN link with the fingerprint of the certificate; this program then needs no TLS at all and speaks the routes of the `printer3d` plugin it speaks already. It leaves the stand-alone mode without Bambu.
* **B. A TLS library behind a feature.** A feature of `p3dp-link` that takes a TLS crate, off by default, with the fingerprint check of `pin` as the only way to accept a certificate. It breaks the rule of no outside crates for that feature and can only be tried in the CI or on a computer with the crates.
* **C. The TLS of the system.** A different small binding for each system (SChannel, Secure Transport, the library of Linux): the most code and the least testable.

Whatever is chosen, the access code of the printer must live in a store that is encrypted at rest, and the cloud of Bambu is not part of the design and, if it is ever added, will be off by default. Nothing here has been tried on a Bambu printer.

## Models

The projects that other programs save as 3MF are read for their metadata only (which program made them, the objects, the plates and a few settings), from the layout of their files as it looks in a package, and nothing is written back; the settings that are not in that short list are not read. The 3MF reader of the mesh takes the mesh objects of the core specification, with units and the transform of the build items. It refuses components, objects in other files (the production extension), and it ignores colours, materials, plates and the settings that other programs store in a package. Writing a 3MF that carries the G-code for a Bambu printer is step H6.

## Resin printers

The catalog has resin (mSLA) printers, with their volume and how they connect, but they take layers and not G-code: the writer refuses them and the drivers do not exist yet. The open packages of layers (`.sl1`) are read by their headers, with an estimate of the exposure, and nothing else: no layer image is decoded, no file is written, and the closed formats of the makers are refused. Binary G-code is read the same way, by its headers, following the public description of the format and tried only on files that the tests make by it: its thumbnails are listed and one can be taken out, but no block of G-code is unpacked and the program writes no binary G-code. Asking the EH3D server to convert a plain G-code file to binary G-code is not done: it needs a route on the server, and the program shows the metadata and the thumbnails of a file instead. The catalog of resins (seven generic ones) holds starting points for the exposures, not the figures of a bottle, and none has been tried on a machine; the resin a part takes is the volume of its mesh with a percentage for the waste, not the volume of the layers.

## The window

It has four tabs (lists with filters, the plan by layers with a slider, the queue of the server with a confirmed start and an emergency stop, shortcuts that can be changed) and a picture mode, and the logic is tested without a screen, but it has not been built or looked at on a screen. The model of the 3D viewer is the crate `p3dp-viewer` (camera, picking, selection box, range of layers, lines of a plan); the drawing with wgpu is behind its feature `gui`, never built, and the window does not use it yet: today the preview is lines. The texts of the panels and controls are in the eight languages. The names of the fifteen printer states are translated in the eight languages.

## The server

The rules for putting a job in the queue (no repeated name, a longest line) are kept by this program before it sends anything and are set in the settings file; the server keeps its own rules, and a job that this program lets through can still be refused there.

The client speaks the routes of the `printer3d` plugin of the server through the common client of the suite, but has only met a double of them in memory: it has never talked to a running `eh3d-server`. The name `phoenix-3dp` is not among the applications the server knows by default, so a login needs it in `auth_extra_apps` (or a token of a program).
