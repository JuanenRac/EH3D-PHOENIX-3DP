//! The project: a model and the printer it was prepared for, kept in the own 3MF package of the program. Packages of other programs (Prusa, Bambu, Orca) are read as
//! models only; what they hold besides the mesh (settings, plates, colours) is not read yet.

use p3dp_core::{Limits, Mesh};
use p3dp_formats::threemf::{self, Meta};
use p3dp_formats::FormatError;

/// A project.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub name: String,
    /// The id of the printer profile; empty when none was chosen.
    pub printer_id: String,
    pub mesh: Mesh,
}

impl Project {
    pub fn new(name: &str, printer_id: &str, mesh: Mesh) -> Project {
        Project {
            name: name.to_string(),
            printer_id: printer_id.to_string(),
            mesh,
        }
    }

    /// The package of the project.
    pub fn to_3mf(&self) -> Result<Vec<u8>, FormatError> {
        threemf::write(
            &self.mesh,
            &Meta {
                title: self.name.clone(),
                printer: self.printer_id.clone(),
            },
        )
    }

    /// Reads a package. The name and the printer come back when the package was written by this program; otherwise they are empty.
    pub fn from_3mf(bytes: &[u8], limits: &Limits) -> Result<Project, FormatError> {
        let mesh = threemf::read(bytes, limits)?;
        let meta = threemf::read_meta(bytes, limits).unwrap_or_default();
        Ok(Project {
            name: meta.title,
            printer_id: meta.printer,
            mesh,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;

    #[test]
    fn a_project_round_trips() {
        let p = Project::new("Calibration cube & more", "bambu-lab-a1", cube(20.0));
        let back = Project::from_3mf(&p.to_3mf().unwrap(), &Limits::default()).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn a_package_of_another_program_is_a_model_without_a_name() {
        let z = include_bytes!("../crates/p3dp-formats/tests/fixtures/cube_deflated.3mf");
        let p = Project::from_3mf(z, &Limits::default()).unwrap();
        assert_eq!(p.mesh.triangles.len(), 12);
        assert_eq!(p.name, "cube");
        assert_eq!(p.printer_id, "");
    }

    #[test]
    fn damage_is_an_error() {
        assert!(Project::from_3mf(b"not a package", &Limits::default()).is_err());
        let p = Project::new("x", "", Mesh::new());
        assert!(p.to_3mf().is_ok());
        assert!(
            Project::from_3mf(&p.to_3mf().unwrap(), &Limits::default()).is_err(),
            "a project with no triangles is not a model"
        );
    }
}
