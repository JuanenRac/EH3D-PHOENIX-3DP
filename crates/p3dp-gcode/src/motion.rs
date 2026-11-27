//! The movements of a G-code program, read to be looked at and checked: the lines each layer lays down and travels, the filament they take and every move that leaves the build
//! volume of a printer. It is a reader and nothing else: no line is changed, run or sent. What it cannot follow (a position that is not known, a move written in a way it does
//! not read) is counted and said, never guessed; a macro with a name of its own is not followed.
//!
//! The volume is the one of a printer with its origin at a corner: from 0 to the size on each axis.

use crate::check::{command_of, program_lines, words_of, Word};
use std::collections::VecDeque;

/// The most lines of the head kept to be drawn.
pub const MAX_SEGMENTS: usize = 300_000;
/// The most layers kept to be drawn.
pub const MAX_LAYERS: usize = 20_000;
/// The most moves past the volume listed one by one.
pub const MAX_OUTSIDE: usize = 100;
/// How far past an edge a move may go before it is told: the rounding of three decimals.
const SLACK_MM: f64 = 0.001;
/// The length of the straight pieces an arc is followed in, in millimetres.
const ARC_PIECE_MM: f64 = 1.0;
/// The most pieces of one arc.
const MAX_ARC_PIECES: usize = 360;

/// A straight line of the head over the bed, in millimetres.
pub type Segment = [[f64; 2]; 2];

/// One layer: the lines that lay plastic and the ones that only move, at one height.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionLayer {
    pub z_mm: f64,
    pub extrusions: Vec<Segment>,
    pub travels: Vec<Segment>,
    /// The filament the lines of the layer take, in millimetres.
    pub filament_mm: f64,
    /// The length of the lines that lay plastic.
    pub length_mm: f64,
}

/// A move that goes past the build volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outside {
    /// The line of the program, starting at 1.
    pub line: usize,
    pub axis: char,
    pub value_mm: f64,
}

/// What the movements of a program come to.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Motion {
    /// The layers in the order they are printed (a program that prints one part after another comes back down, and those are layers of their own).
    pub layers: Vec<MotionLayer>,
    /// The first moves past the volume.
    pub outside: Vec<Outside>,
    /// How many lines go past the volume.
    pub outside_count: usize,
    /// How many moves could not be followed, so nothing is said about where they go.
    pub unfollowed: usize,
    /// The filament of the lines that lay plastic, in millimetres.
    pub filament_mm: f64,
    /// How many lines were not kept to be drawn (they are checked all the same).
    pub not_kept: usize,
}

struct State {
    volume_mm: Option<[f64; 3]>,
    absolute: bool,
    absolute_e: bool,
    /// Millimetres in a unit of the program.
    unit: f64,
    /// Where the head is, from the corner of the bed; `None` while it is not known.
    at: [Option<f64>; 3],
    /// What `G92` added: the place is the number of the program plus this.
    offset: [f64; 3],
    /// A new origin was given where the place was not known: the numbers of that axis mean nothing until it is homed.
    lost: [bool; 3],
    e: f64,
    segments: usize,
    /// The travels since the last line that laid plastic: they belong to the layer of the next one.
    pending: VecDeque<Segment>,
    out: Motion,
}

fn value(words: &[Word], letter: char) -> Option<Result<f64, ()>> {
    words.iter().skip(1).find(|w| w.0 == letter).map(|w| w.1)
}

impl State {
    fn judge(&mut self, line: usize, at: [Option<f64>; 3], told: &mut bool) {
        let Some(volume) = self.volume_mm else {
            return;
        };
        for (i, axis) in ['X', 'Y', 'Z'].into_iter().enumerate() {
            let Some(v) = at[i] else { continue };
            if !*told && (v < -SLACK_MM || v > volume[i] + SLACK_MM) {
                *told = true;
                self.out.outside_count += 1;
                if self.out.outside.len() < MAX_OUTSIDE {
                    self.out.outside.push(Outside {
                        line,
                        axis,
                        value_mm: v,
                    });
                }
            }
        }
    }

    fn lay(&mut self, z: f64, segment: Segment, filament: f64) {
        let same = self
            .out
            .layers
            .last()
            .is_some_and(|l| (l.z_mm - z).abs() < 1e-6);
        if !same {
            if self.out.layers.len() >= MAX_LAYERS {
                self.out.not_kept += 1;
                return;
            }
            self.out.layers.push(MotionLayer {
                z_mm: z,
                extrusions: Vec::new(),
                travels: Vec::new(),
                filament_mm: 0.0,
                length_mm: 0.0,
            });
        }
        let Some(layer) = self.out.layers.last_mut() else {
            return;
        };
        layer.travels.extend(self.pending.drain(..));
        let (dx, dy) = (segment[1][0] - segment[0][0], segment[1][1] - segment[0][1]);
        layer.length_mm += dx.hypot(dy);
        layer.filament_mm += filament;
        if self.segments < MAX_SEGMENTS {
            self.segments += 1;
            layer.extrusions.push(segment);
        } else {
            self.out.not_kept += 1;
        }
    }

    fn travel(&mut self, segment: Segment) {
        if self.segments < MAX_SEGMENTS {
            self.segments += 1;
            self.pending.push_back(segment);
        } else {
            self.out.not_kept += 1;
        }
    }

    /// One straight piece from where the head is to `to`, with the filament it takes.
    fn piece(&mut self, line: usize, to: [Option<f64>; 3], e: f64, told: &mut bool) {
        self.judge(line, to, told);
        let from = self.at;
        self.at = to;
        if let (Some(x0), Some(y0), Some(x1), Some(y1)) = (from[0], from[1], to[0], to[1]) {
            if (x1 - x0).hypot(y1 - y0) < 1e-9 {
                return;
            }
            let segment = [[x0, y0], [x1, y1]];
            match to[2] {
                Some(z) if e > 1e-9 => {
                    self.out.filament_mm += e;
                    self.lay(z, segment, e);
                }
                _ => self.travel(segment),
            }
        } else if e > 1e-9 {
            self.out.unfollowed += 1;
        }
    }

    fn movement(&mut self, line: usize, words: &[Word], clockwise: Option<bool>) {
        let mut to = self.at;
        let mut moves = false;
        let mut followed = true;
        for (i, letter) in ['X', 'Y', 'Z'].into_iter().enumerate() {
            let Some(found) = value(words, letter) else {
                continue;
            };
            moves = true;
            let Ok(v) = found else {
                to[i] = None;
                followed = false;
                continue;
            };
            let v = v * self.unit;
            to[i] = if self.absolute {
                (!self.lost[i]).then_some(v + self.offset[i])
            } else {
                self.at[i].map(|p| p + v)
            };
            followed &= to[i].is_some();
        }
        let mut e = 0.0;
        if let Some(Ok(v)) = value(words, 'E') {
            let v = v * self.unit;
            e = if self.absolute_e { v - self.e } else { v };
            self.e = if self.absolute_e { v } else { self.e + v };
        }
        if !followed {
            self.out.unfollowed += 1;
        }
        let mut told = false;
        let (Some(clockwise), true) = (clockwise, followed) else {
            if moves {
                self.piece(line, to, e, &mut told);
            }
            return;
        };
        // an arc, given by the place of its centre from where it starts
        let part = |letter| {
            value(words, letter)
                .and_then(Result::ok)
                .map(|v| v * self.unit)
        };
        let (i, j) = (part('I'), part('J'));
        let (Some(x0), Some(y0), Some(x1), Some(y1), false) = (
            self.at[0],
            self.at[1],
            to[0],
            to[1],
            i.is_none() && j.is_none(),
        ) else {
            self.out.unfollowed += 1;
            self.piece(line, to, e, &mut told);
            return;
        };
        let (cx, cy) = (x0 + i.unwrap_or(0.0), y0 + j.unwrap_or(0.0));
        let radius = (x0 - cx).hypot(y0 - cy);
        let start = (y0 - cy).atan2(x0 - cx);
        let end = (y1 - cy).atan2(x1 - cx);
        let turn = std::f64::consts::TAU;
        let mut sweep = (end - start).rem_euclid(turn);
        if sweep < 1e-9 {
            sweep = turn;
        }
        if clockwise {
            sweep -= turn;
            if sweep > -1e-9 {
                sweep = -turn;
            }
        }
        let pieces =
            ((sweep.abs() * radius / ARC_PIECE_MM).ceil() as usize).clamp(1, MAX_ARC_PIECES);
        for k in 1..=pieces {
            let point = if k == pieces {
                to
            } else {
                let a = start + sweep * k as f64 / pieces as f64;
                [
                    Some(cx + radius * a.cos()),
                    Some(cy + radius * a.sin()),
                    to[2],
                ]
            };
            self.piece(line, point, e / pieces as f64, &mut told);
        }
    }

    fn command(&mut self, line: usize, words: &[Word]) {
        let Some(&(letter, Ok(number))) = words.first() else {
            return;
        };
        if number.fract() != 0.0 {
            return;
        }
        match (letter, number as i64) {
            ('G', 0 | 1) => self.movement(line, words, None),
            ('G', 2) => self.movement(line, words, Some(true)),
            ('G', 3) => self.movement(line, words, Some(false)),
            ('G', 20) => self.unit = 25.4,
            ('G', 21) => self.unit = 1.0,
            ('G', 28) => {
                let named = ['X', 'Y', 'Z'].map(|l| value(words, l).is_some());
                for (i, one) in named.iter().enumerate() {
                    // where an axis ends up after homing is a matter of the machine
                    if *one || named == [false; 3] {
                        self.at[i] = None;
                        self.offset[i] = 0.0;
                        self.lost[i] = false;
                    }
                }
            }
            ('G', 90) => {
                self.absolute = true;
                self.absolute_e = true;
            }
            ('G', 91) => {
                self.absolute = false;
                self.absolute_e = false;
            }
            ('M', 82) => self.absolute_e = true,
            ('M', 83) => self.absolute_e = false,
            ('G', 92) => {
                for (i, letter) in ['X', 'Y', 'Z'].into_iter().enumerate() {
                    let Some(Ok(v)) = value(words, letter) else {
                        continue;
                    };
                    match self.at[i] {
                        Some(p) => self.offset[i] = p - v * self.unit,
                        None => self.lost[i] = true,
                    }
                }
                if let Some(Ok(v)) = value(words, 'E') {
                    self.e = v * self.unit;
                }
            }
            _ => {}
        }
    }
}

/// Reads the movements of a program. With the build volume of a printer, every line that goes past it is counted and the first ones are listed; without it nothing is judged.
pub fn read_motion(text: &str, volume_mm: Option<[f64; 3]>) -> Motion {
    let mut state = State {
        volume_mm,
        absolute: true,
        absolute_e: true,
        unit: 1.0,
        at: [None; 3],
        offset: [0.0; 3],
        lost: [false; 3],
        e: 0.0,
        segments: 0,
        pending: VecDeque::new(),
        out: Motion::default(),
    };
    for (i, raw) in program_lines(text).enumerate() {
        let code = command_of(raw);
        // only the classic commands move: a macro with a name of its own is not followed
        let mut first = code.chars();
        let classic = first
            .next()
            .is_some_and(|c| matches!(c, 'G' | 'g' | 'M' | 'm'))
            && first
                .find(|c| !c.is_ascii_whitespace())
                .is_some_and(|c| c.is_ascii_digit());
        if !classic {
            continue;
        }
        let words = words_of(code);
        for command in crate::check::commands(&words) {
            state.command(i + 1, command);
        }
    }
    if let Some(layer) = state.out.layers.last_mut() {
        layer.travels.extend(state.pending.drain(..));
    }
    state.out
}
