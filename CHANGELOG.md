# Changelog

All notable changes to this project are documented here.

## [Unreleased]

- A catalog of 46 printer profiles written from public facts (volume, nozzles, dialect, ways of connecting, multi-material system and the temperature limits the maker states), FDM and resin, among them the Bambu Lab A1 and P1S, the Prusa MK4S, Core One and MINI+, the Creality Ender-3 V3 and K1 families, Voron and generic Klipper, Anycubic, Elegoo and Qidi; every profile says `verified = false`, and a profile can be written by hand in a file of the same kind.
- Readers of STL (text and binary), OBJ and basic 3MF, with a ZIP and inflate reader written from the specification, limits on the size, the triangles, the entries and what a package inflates to, and refusals that name what was found; writers of binary STL, OBJ, an own 3MF package and G-code files.
- G-code writers for the Marlin, Klipper, RepRapFirmware and Bambu dialects with a safe header, start and end, where a temperature over the limit of the model is refused and nothing is written, and a checker that finds the lines of any program that go past the limits.
- A driver trait with one normalized state of fifteen states, Moonraker and OctoPrint drivers over a transport, in-memory simulators of both protocols with faults on demand, the retry rules of the suite and the design of the Bambu LAN link marked experimental.
- The settings of the link to the EH3D server with the ranges of the common client, a thin client of the draft routes of the printer plugin that never asks a job to start by itself, and a server in memory to test it.
- A command line with `printers`, `info`, `check-profile`, `convert` and `version`, the texts of the program in eight languages, and the base of a window with four tabs (Prepare, Preview, Device, Project) for the gui feature, whose logic is tested without a screen.
- Tests with a fixed seed, fuzz tests of every reader, tests that no temperature of any printer of the catalog goes past its limit, and tests that the READMEs state the real number of tests.

## [0.0.1] - The frame of the workspace

- The Cargo workspace, the CI, the manifest, the documentation in eight languages and the community files of a new repository of the suite.
