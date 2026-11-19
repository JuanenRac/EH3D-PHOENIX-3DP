# Roadmap

Nine steps from the base that exists today to a complete program. Each step ends with its tests and its documentation up to date, and nothing is claimed about a real printer without evidence from a real printer. The state of each step is told honestly: nothing is green until it has run on a machine.

| Step | What it is | State |
|---|---|---|
| H0 | The frame and the base: the workspace, the CI, the units, the mesh and its checks, the readers and writers of the formats, the catalog of profiles, the G-code writers with limits that refuse, the driver trait and the Moonraker and OctoPrint drivers against simulators, the thin link to the server, the command line and the base of the window | done, with simulators only |
| H1 | Mesh and geometry: more operations on the mesh, repair of what can be repaired, and a 3D viewer of its own (wgpu) with the plate and the orbit | not started |
| H2 | A minimal slicer: slices, regions, classic perimeters, rectilinear and grid infill, top and bottom layers, G-code for Marlin and Klipper, an estimate of the time, a command `slice`, and a calibration cube checked against the geometry | not started |
| H3 | Profiles and the base interface: the editor of profiles with inheritance (printer, filament, process), the list of objects, plates, and the sync of profiles with the server | not started |
| H4 | Quality: variable width of the lines, the position of the seam, ironing, fuzzy skin, bridges and overhangs, retraction and z-hop, pressure advance, brim, skirt and raft, vase mode, variable layer height, arc fitting | not started |
| H5 | Supports and advanced infill: normal and tree supports, painting of supports and seams, gyroid, honeycomb, cubic, lightning and adaptive infill, and a viewer of the G-code by layers with its types and speeds | not started |
| H6 | Multi-material: purge tower and purge map, grouping by nozzle, toolchangers and dual extruders, painting of colours, reading 3MF with plates, and writing the G-code package that the Bambu printers take | not started |
| H7 | Connectivity and the server: network transports for Moonraker and OctoPrint, PrusaLink and Duet, the queue in the server, the state in real time, the camera, the checks before printing, the reports; the Bambu LAN link as an experimental option | not started (the drivers and the thin client exist against simulators) |
| H8 | Guided calibrations, a simulator or twin, profiles that have evidence from real machines, and the simulation of collisions | not started |
| H9 | Polish: configurable shortcuts, themes, all eight languages complete (the names of the states are not translated yet), mobile and touch-screen reading of results, the documentation, the screenshots and a signed package | not started |

## What is decided

* The EH3D server and its clients are the centre: the printers, the queue and the permissions live in the server, and this program is one more client. The stand-alone mode that talks straight to a printer is optional and off by default.
* FDM and resin (mSLA) printers are both in the catalog from the start; the drivers for resin printers come later.
* Moonraker/Klipper and OctoPrint come first, with simulators; Bambu LAN is experimental; the cloud of Bambu is optional and off.
* The queue never starts a job by itself unless the policy of the server says so, and the default is off.
* The temperature limits are the ones that the maker states for each model, kept in a profile that the owner can edit.
* Slicing may also run in the server, as an option.
* Projects: read the packages of other programs as models, write the own 3MF and, later, a 3MF with G-code that the printers that need one accept.
* The viewer is written for this program (wgpu); the shortcuts of the family are the default, with a scheme of its own.
* The name carries a notice about the marks of third parties, and nothing is copied from any other slicer: the program is written from public protocols, manuals and facts.
