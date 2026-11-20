# Changelog

All notable changes to this project are documented here.

## [Unreleased]

- The CI is built against EH3D-COMMON v0.2.5 in the three workflows that name the tag of a sibling.
- The client of the printer plugin sets the target of a heater (`hotend`, `bed` or `chamber`, checked here against the stricter limits before anything is sent, and again by the server) and reads the answers shaped like Moonraker's and OctoPrint's when the machine has them on; the server in memory gives the same refusals as the real one (`temperature_over_limit`, `bad_heater`, `not_supported`).

## [0.0.2] - Server client, TCP transport for printers, a minimal slicer and reading of projects from other programs

- The client of the server is now the common client of the suite (`eh3d-serverclient` of EH3D-COMMON, taken by path from a sibling folder, with the tag it is built against in the workflows): login and renewal, retries that never send a POST twice, the state of the link and every refusal of the server in eight languages. It speaks the routes of the `printer3d` plugin of the server (machines, jobs, temperatures, layer, emergency stop, release, making a machine for a model), checks a program against the stricter of the limits of the profile and the limits the server reports before anything is sent, and only queues a job: starting it needs a confirmation that stands for a person.
- A transport over TCP for the Moonraker and OctoPrint drivers, written with the standard library: plain HTTP/1.1, one request per connection, a limit of time to connect and to each read, limits on the head and the body of an answer, chunked answers, no redirect followed, no TLS (refused with the reason), and no byte of a request that could start another one. Tested against a socket of the same computer, where the two simulated printers also run a whole job over a real connection.
- A minimal slicer (`p3dp-slicer`): the cut of a closed mesh by flat layers, perimeters (the loops that an offset folds over itself are cut apart), a rectilinear infill with solid skins where the layers above or below do not cover, G-code for the Marlin, Klipper, RepRapFirmware and Bambu dialects through the writer, the plan of paths by layer as data for a preview, an estimate of the time and the new command `slice`. Every value is in a settings file with a range; a temperature over the limit, a model that does not fit the bed, a mesh that is not closed or inside out are refused, never adjusted. Tested with properties on shapes from a fixed seed: the plastic laid down is the volume of the part, nothing leaves the bed, the head only goes up and no temperature goes past a limit.
- A read-only reader of the 3MF projects that other programs save (`project3mf`) and the command `project`: which program made the package, its objects, plates (with the predicted time and weight when the package says) and a few settings, with limits against packages made to hurt (entries, inflated size, the size of the metadata, the number of objects and plates) and fuzz tests.
- The names of the fifteen states of a printer in the eight languages (the window showed the key before).
- A catalog of 46 printer profiles written from public facts (volume, nozzles, dialect, ways of connecting, multi-material system and the temperature limits the maker states), FDM and resin, among them the Bambu Lab A1 and P1S, the Prusa MK4S, Core One and MINI+, the Creality Ender-3 V3 and K1 families, Voron and generic Klipper, Anycubic, Elegoo and Qidi; every profile says `verified = false`, and a profile can be written by hand in a file of the same kind.
- Readers of STL (text and binary), OBJ and basic 3MF, with a ZIP and inflate reader written from the specification, limits on the size, the triangles, the entries and what a package inflates to, and refusals that name what was found; writers of binary STL, OBJ, an own 3MF package and G-code files.
- G-code writers for the Marlin, Klipper, RepRapFirmware and Bambu dialects with a safe header, start and end, where a temperature over the limit of the model is refused and nothing is written, and a checker that finds the lines of any program that go past the limits.
- A driver trait with one normalized state of fifteen states, Moonraker and OctoPrint drivers over a transport, in-memory simulators of both protocols with faults on demand, the retry rules of the suite and the design of the Bambu LAN link marked experimental.
- The settings of the link to the EH3D server with the ranges of the common client, a thin client of the draft routes of the printer plugin that never asks a job to start by itself, and a server in memory to test it.
- A command line with `printers`, `info`, `check-profile`, `convert` and `version`, the texts of the program in eight languages, and the base of a window with four tabs (Prepare, Preview, Device, Project) for the gui feature, whose logic is tested without a screen.
- Tests with a fixed seed, fuzz tests of every reader, tests that no temperature of any printer of the catalog goes past its limit, and tests that the READMEs state the real number of tests.

## [0.0.1] - The frame of the workspace

- The Cargo workspace, the CI, the manifest, the documentation in eight languages and the community files of a new repository of the suite.
