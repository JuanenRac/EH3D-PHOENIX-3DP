; a small program that keeps to the limits of the profile in my-printer.cfg
PRINT_START BED=60 EXTRUDER=215
G1 X10 Y10 Z0.2 E0.5 F1200
M104 S210
M140 S60
PRINT_END
