# EH3D-PHOENIX-3DP: not yet

Good ideas that are not in the current version. Writing them here keeps anybody from implementing them early or twice.

| Idea | Why not yet | Revisit when |
|---|---|---|
| A real printer as the first proof | No driver has spoken to a real printer; the proof needs a person at a printer with the power switch in reach | the owner has a printer ready for the first print |
| A connector over a TLS library, for the stand-alone Bambu link and for https addresses | The seam, the pinned channel and the driver exist and run against a printer made in memory; the connector itself needs a TLS crate, which would live outside this workspace because it is not in the offline cache | the owner wants the stand-alone Bambu link |
| Painting of colours on a surface, the offsets of the heads of a toolchanger and the grouping of filaments by nozzle | A tool changes between layers, with a purge tower; nothing is painted on a surface and the heads of a toolchanger have no offsets | the change of tool and the tower have printed on a real printer |
| Tree supports, interface layers and adaptive layer height | The slicer has plain supports, the ironing of the top, the pressure advance and the usual quality settings; the rest is not started | the plain supports have printed well on a real printer |
| A driver for resin printers | The catalog lists resin printers, but they take layers and not G-code; the writer refuses them and no driver exists | the format of the layers of a printer is chosen |
| The WebSocket push of Moonraker and OctoPrint | The drivers poll over plain HTTP; a push needs a WebSocket client written by hand | the polling is too slow for the window |
| PrusaLink and Duet as drivers | The state tables exist; the drivers are not written | a printer of those families is in the workshop |
| Writing profiles and projects for other programs, and the settings of single objects, filaments by slot and supports of an imported project | The import brings the settings the slicer has, one value each, and lists the rest; nothing is written for another program | the import has been used with real files of the person |
| Guided calibrations and a simulator of collisions | They need the real printers first | a printer has been verified |
| Several plates, several filaments and the other files of the printer package | The package carries one plate and the facts of one filament; the rest of the layout is not published | a package has printed on a real printer |
| The cloud of Bambu | The design is local and through the server; a cloud would be off by default and is not part of it | the owner asks for it |

## Details by area

This page lists what the program does not do, and what each missing part needs. Nothing on it is hidden in the README: the table "Project status" says the same with lights.

### A small slicer

The program turns a closed model into flat layers, perimeters, a rectilinear, grid or gyroid infill with solid skins, bridges, plain supports, seams, variable width of the perimeters and G-code, and shows the plan by layers as data. It has no tree supports, interface layers or painting of supports (it does follow the lean of the walls with the height of the layers, draw in the first layer against the flared foot and hold every line to a limit of the flow), and its several materials change tool between layers only, with a purge tower, and never inside a layer (it does iron the top layer, write the pressure advance, lay a skirt, a brim or a raft, slow a short layer, shorten the retraction of a short travel and print a vase as one wall of separate layers, not as a spiral); a bridge is laid across the gap but is not tuned (no flow test, no extra anchor); the supports are lines at a pitch, with no interface; the time it estimates has an acceleration and a speed at the corners from the settings, not the planner of the printer; and no print made from its G-code has been run on a printer. A model that is not closed, that faces inwards or that does not fit is refused, not repaired or scaled.

### Nothing has run on a printer

* No driver has spoken to a real printer. The Moonraker and OctoPrint drivers follow the public description of the protocols and have been tried only against the simulators of this repository, which are written by us and prove the logic, not that a real server answers the same way.
* Every profile of the catalog says `verified = false`. A profile becomes verified when someone has printed with it on that model and says so (an issue with the form "Hardware validation result"). A Bambu Lab A1 and a P1S are the first printers planned for that.
* The temperature limits are the ones that the makers state in their data sheets. They have not been compared with a printer, and a wrong limit in the catalog is a bug to report.

### Network

The drivers talk to an `HttpTransport`; `TcpTransport` is a plain HTTP/1.1 one written with the standard library, tried only against a socket of the same computer. It has no TLS (an `https://` address is refused), follows no redirect and keeps no connection open. The WebSocket push of Moonraker and the push of OctoPrint are step H7. The retry rules (a refused connection is tried again for any request; a time-out only for requests that can be sent twice; never a POST) are tested against the simulators.

### Bambu Lab

`p3dp-link/src/bambu.rs` holds what can be written and tested without a printer: the table of states, the names of the topics, the messages that ask for the whole state and that pause, resume or stop, and the settings of the local link (`LanSettings`), which are refused without the fingerprint of the certificate of the printer. Around it are three modules with the protocol as bytes and text, tried with a broker made in memory and with the published test vectors: `mqtt` (the packets of MQTT 3.1.1 that the link uses, quality 0 only), `ftps` (the commands and replies of the upload, with the check that the data connection goes to the address of the control connection) and `pin` (SHA-256 and the comparison of the fingerprint of a certificate). `tls` is the seam where a TLS connection plugs in (a connector, a channel, a channel that only exists after the pinned certificate was compared, a printer made in memory behind them) and `bambu_lan` is a driver over it that reads the state and pauses, resumes and stops a print; `BambuDriver` still answers `Experimental` to everything and the driver over the seam has only been tried against the printer made in memory, because nothing opens a real TLS connection: the standard library has no TLS and this repository takes no crates from outside. The upload of a file and the start of a print are not built.

Pending, with the ways to close it:

* **A. Through the EH3D server (chosen).** The server has the experimental Bambu LAN link with the fingerprint of the certificate; this program then needs no TLS at all and speaks the routes of the `printer3d` plugin it speaks already. The client makes the machine with the fingerprint pinned and tells the troubles of the link by their codes (`SERVER.md`). It leaves the stand-alone mode without Bambu.
* **B. A TLS library behind the seam.** A small crate outside this workspace that implements `TlsConnector` over a TLS library, with the fingerprint check of `pin` (through `connect_pinned`) as the only way to accept a certificate. The seam, the pinned channel and the driver are written, so the crate is the only part missing, and it keeps the offline cache of the CI untouched; it can only be tried in a CI or on a computer with the crates.
* **C. The TLS of the system.** A different small binding for each system (SChannel, Secure Transport, the library of Linux): the most code and the least testable.

Whatever is chosen, the access code of the printer must live in a store that is encrypted at rest, and the cloud of Bambu is not part of the design and, if it is ever added, will be off by default. Nothing here has been tried on a Bambu printer.

### Models

The projects that other programs save as 3MF are read for their metadata only (which program made them, the objects, the plates and a few settings), from the layout of their files as it looks in a package, and nothing is written back; the settings that are not in that short list are not read by that command (`import` reads every setting of the project and says where each one ended). The 3MF reader of the mesh takes the mesh objects of the core specification, with units and the transform of the build items. It refuses components, objects in other files (the production extension), and it ignores colours, materials, plates and the settings that other programs store in a package. A 3MF package that carries the G-code of one plate for a Bambu printer (`.gcode.3mf`) is written from the 3MF and packaging standards; the maker does not publish its whole layout, and no printer has been given one.

### Resin printers

The catalog has resin (mSLA) printers, with their volume and how they connect, but they take layers and not G-code: the writer refuses them and the drivers do not exist yet. The open packages of layers (`.sl1`) are read by their headers, with an estimate of the exposure, and nothing else: no layer image is decoded, no file is written, and the closed formats of the makers are refused. Binary G-code is read the same way, by its headers, following the public description of the format and tried only on files that the tests make by it: its thumbnails are listed and one can be taken out, but no block of G-code is unpacked and the program writes no binary G-code. Asking the EH3D server to convert a plain G-code file to binary G-code is not done: it needs a route on the server, and the program shows the metadata and the thumbnails of a file instead. The catalog of resins (seven generic ones) holds starting points for the exposures, not the figures of a bottle, and none has been tried on a machine; the resin a part takes is the volume of its mesh with a percentage for the waste, not the volume of the layers.

### The window

It has a window in the docking frame of the suite, with the four old tabs as workspaces (Prepare, Review, Run and Setup) and panels that can be taken away, moved, floated and folded, with a menu bar (lists with filters, the bed in 3D with the parts as shaded solids and their numbers, auto-orient and arrange, the plan by layers with a slider and the estimates, the queue of the server with a confirmed start and an emergency stop, shortcuts that can be changed, a dark and a light look, temperature limits that can be edited) and a picture mode, and the logic is tested without a screen, but it has not been looked at on a screen: the frame was run in a context with no window in the tests of the suite, and the panels of this program were type-checked and linted but never drawn. The pictures of the README for the workspaces are drawings from the model of the arrangement, not photographs; the photographs of the window are to be taken by the manual workflow `Window pictures`, which has not been run. The files are named by typing a path (there is no window to choose a file yet), and there is no undo. A part cannot be dragged over the bed with the pointer yet (its place is typed), the vertical slider of the layers and the thumbnail are drawn by hand, and the progress bar of the slice is an estimate because the slicer does not report its steps. The Device tab always says that the printer plugin of the server is a simulator. The model of the 3D viewer is the crate `p3dp-viewer` (camera, picking, selection box, range of layers, lines of a plan); the drawing with wgpu is behind its feature `gui`, never built, and the window does not use it yet: today the preview is lines. The texts of the panels and controls are in the eight languages. The names of the fifteen printer states are translated in the eight languages.

### The server

The rules for putting a job in the queue (no repeated name, a longest line) are kept by this program before it sends anything and are set in the settings file; the server keeps its own rules, and a job that this program lets through can still be refused there.

The client speaks the routes of the `printer3d` plugin of the server through the common client of the suite, but has only met a double of them in memory: it has never talked to a running `eh3d-server`. The name `phoenix-3dp` is not among the applications the server knows by default, so a login needs it in `auth_extra_apps` (or a token of a program).

## The project of the suite

The branch `3dp/` of a `.eh3d` project keeps meshes, placements, settings, G-code and the start and end G-code of the person. It does not keep the profiles of printers and filaments of the person nor the limits typed for a printer (a project of somebody else must not raise a temperature limit); resin work is not kept apart; STEP files taken from another branch are kept but not read; the G-code of a project is shown and checked for temperatures, but it is not drawn as layers nor sent, and its moves are not checked against the build volume. Nothing of the window of the project or of the panel of the attachments has been tried on a screen by a person.
