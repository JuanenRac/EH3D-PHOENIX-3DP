//! Why a model was not sliced. Every one of these is a refusal: the slicer never changes a model, a setting or a temperature to make it pass.

use p3dp_core::machine::TempError;
use p3dp_core::CoreError;
use p3dp_gcode::writer::GcodeError;
use std::fmt;

/// What can stop a slice.
#[derive(Debug, Clone, PartialEq)]
pub enum SliceError {
    /// A setting that is out of its range, or settings that do not go together.
    Config(String),
    /// A temperature above what the model allows.
    Temp(TempError),
    /// The printer cannot be sliced for (a resin printer takes layers of an image, and the profile has no limits to keep to).
    Machine(String),
    /// The mesh has nothing in it.
    EmptyMesh,
    /// The mesh is not closed: the edges that are not shared by exactly two triangles.
    OpenMesh { open_edges: usize },
    /// The triangles face inwards.
    InvertedMesh,
    /// The model is larger than the printer, or does not fit with the margin asked for.
    DoesNotFit { what: String },
    /// A layer whose cut does not close up (a mesh that looks closed but is not manifold).
    OpenLoop { layer: usize },
    /// More layers than the limit.
    TooManyLayers { layers: usize, max: usize },
    /// More points in one layer than the limit.
    TooManyPoints {
        layer: usize,
        points: usize,
        max: usize,
    },
    /// A part of a layer is thinner than the perimeters and `thin_regions = refuse`.
    ThinRegion { layer: usize },
    /// More lines of G-code than the limit.
    TooManyLines { lines: usize, max: usize },
    /// A bridge in the air longer than `max_bridge_span_mm` and no supports to hold it.
    BridgeTooLong {
        layer: usize,
        span_mm: f64,
        max_mm: f64,
    },
    /// An overhang that would have to stand on the part itself while the supports are asked to stand on the bed only.
    UnsupportedOverhang { layer: usize },
    /// The decision on the supports would take more samples than the limit.
    TooManySupportSamples { samples: u64, max: u64 },
    /// A vase is one outline on every layer above its base.
    VaseNeedsOneOutline { layer: usize, outlines: usize },
    /// The head would be lifted past the height of the printer.
    LiftPastTop { needed_mm: f64, height_mm: f64 },
    /// The writer of G-code refused something.
    Gcode(GcodeError),
    /// A number of the model that cannot be used.
    Core(CoreError),
}

impl fmt::Display for SliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SliceError::Config(s) | SliceError::Machine(s) => f.write_str(s),
            SliceError::Temp(t) => write!(f, "{t}"),
            SliceError::EmptyMesh => f.write_str("the model has no triangles"),
            SliceError::OpenMesh { open_edges } => write!(
                f,
                "the model is not closed ({open_edges} edges are not shared by two triangles): repair it first"
            ),
            SliceError::InvertedMesh => f.write_str("the triangles of the model face inwards"),
            SliceError::DoesNotFit { what } => write!(f, "the model does not fit: {what}"),
            SliceError::OpenLoop { layer } => {
                write!(f, "the cut of layer {layer} does not close up")
            }
            SliceError::TooManyLayers { layers, max } => {
                write!(f, "{layers} layers, more than the limit of {max}")
            }
            SliceError::TooManyPoints { layer, points, max } => write!(
                f,
                "layer {layer} has {points} points, more than the limit of {max}"
            ),
            SliceError::ThinRegion { layer } => write!(
                f,
                "layer {layer} has a part thinner than the perimeters (thin_regions = refuse)"
            ),
            SliceError::TooManyLines { lines, max } => {
                write!(f, "{lines} lines of G-code, more than the limit of {max}")
            }
            SliceError::BridgeTooLong {
                layer,
                span_mm,
                max_mm,
            } => write!(
                f,
                "layer {layer} has a bridge {span_mm:.1} mm long over nothing, more than max_bridge_span_mm {max_mm}: turn the supports on or change the part"
            ),
            SliceError::UnsupportedOverhang { layer } => write!(
                f,
                "an overhang at layer {layer} would have to stand on the part itself, and the supports are limited to the bed (supports = buildplate)"
            ),
            SliceError::TooManySupportSamples { samples, max } => write!(
                f,
                "the supports would need {samples} samples, more than the limit of {max} (raise support_spacing_mm)"
            ),
            SliceError::LiftPastTop {
                needed_mm,
                height_mm,
            } => write!(
                f,
                "the print and the z hop need {needed_mm:.2} mm of height and the printer has {height_mm} mm"
            ),
            SliceError::VaseNeedsOneOutline { layer, outlines } => write!(
                f,
                "layer {layer} has {outlines} outlines: a vase is one wall round one outline (vase_mode = off prints the part as it is)"
            ),
            SliceError::Gcode(e) => write!(f, "{e}"),
            SliceError::Core(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SliceError {}

impl From<GcodeError> for SliceError {
    fn from(e: GcodeError) -> SliceError {
        match e {
            GcodeError::Temp(t) => SliceError::Temp(t),
            other => SliceError::Gcode(other),
        }
    }
}

impl From<TempError> for SliceError {
    fn from(e: TempError) -> SliceError {
        SliceError::Temp(e)
    }
}

impl From<CoreError> for SliceError {
    fn from(e: CoreError) -> SliceError {
        SliceError::Core(e)
    }
}
