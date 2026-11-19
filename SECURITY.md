# Security policy

EH3D-PHOENIX-3DP reads files that other people may send (models, packages, settings, profiles) and answers of printers and servers. It never panics on them and refuses a value that makes no sense, but it has had no independent review.

What it does **not** do: it does not know the real limits of your machine, and it does not decide whether what it produces is safe to run. The temperature limits of the profiles are the ones that the makers state for each model; every profile says `verified = false` until someone has tried it on a machine. A result that was produced without errors is not a verified result: a person must check it before anything is run on a printer.

What it enforces: a temperature above the limit of the model is refused (never cut down), in the writers of G-code, in the drivers and in the checker of programs; raw G-code is switched off by default and, when on, is one line at a time and keeps to the same limits; a job put in the queue of the server never asks to start by itself.

Secrets: the token of the server and the keys of the printers are held in a type that prints `***`; the settings are shown with the token hidden. Nothing is sent over the network by this version: the drivers have been tried only against simulators.

Please report a vulnerability privately to **electrohobby3d@gmail.com**, with the file or the steps that show it. Do not open a public issue for it.
