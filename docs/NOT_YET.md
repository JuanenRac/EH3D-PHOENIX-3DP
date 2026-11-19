# What is not done yet

This page lists what the program does not do, and what each missing part needs. Nothing on it is hidden in the README: the table "Project status" says the same with lights.

## No slicer

The program reads a model and checks it, but it does not turn it into layers or into a program for a printer. The G-code writers write the start, the end, the temperatures and the moves that someone asks for, and the checker reads a program; the toolpaths are step H2 of the roadmap.

## Nothing has run on a printer

* No driver has spoken to a real printer. The Moonraker and OctoPrint drivers follow the public description of the protocols and have been tried only against the simulators of this repository, which are written by us and prove the logic, not that a real server answers the same way.
* Every profile of the catalog says `verified = false`. A profile becomes verified when someone has printed with it on that model and says so (an issue with the form "Hardware validation result"). A Bambu Lab A1 and a P1S are the first printers planned for that.
* The temperature limits are the ones that the makers state in their data sheets. They have not been compared with a printer, and a wrong limit in the catalog is a bug to report.

## Network

The drivers talk to an `HttpTransport` and this version has none that opens a socket. A transport for plain HTTP, the WebSocket push of Moonraker and the push of OctoPrint are step H7. The retry rules (a refused connection is tried again for any request; a time-out only for requests that can be sent twice; never a POST) are tested against the simulators.

## Bambu Lab

`p3dp-link/src/bambu.rs` holds what can be written without a printer: the table of states, the names of the topics, the messages that ask for the whole state and that pause, resume or stop. The driver itself answers `Experimental` to everything. It needs, before it connects: a client of MQTT over TLS with the certificate of the printer checked, the upload by FTPS, and the access code of the printer in a store that is encrypted at rest. The cloud of Bambu is not part of the design and, if it is ever added, will be off by default.

## Models

The 3MF reader takes the mesh objects of the core specification, with units and the transform of the build items. It refuses components, objects in other files (the production extension), and it ignores colours, materials, plates and the settings that other programs store in a package. Writing a 3MF that carries the G-code for a Bambu printer is step H6.

## Resin printers

The catalog has resin (mSLA) printers, with their volume and how they connect, but they take layers and not G-code: the writer refuses them and the drivers do not exist yet.

## The window

It has four tabs and its logic is tested, but it has not been built or looked at on a screen. The 3D viewer with a graphics card (wgpu) is step H1; today the preview is lines. The names of the printer states are not translated yet (the keys are shown).

## The server

The routes under `/api/v1/printer3d/` that the thin client uses are a draft: the server does not serve them yet, and the client has only met the server in memory of this repository. When the common client of the suite is taken in, the thin client becomes a call to it.
