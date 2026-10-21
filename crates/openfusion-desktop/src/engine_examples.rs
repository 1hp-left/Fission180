//! Editable designs for exercising the native engine through the ordinary app worker.
use fission180_core::{
    Document, Extrusion, Operation, Profile, Result, Sketch,
    features::{Feature, FeatureKind},
    kernel::Backend,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Example {
    Notch,
    Join,
    Intersect,
    Split,
    Annular,
    Pockets,
}
impl Example {
    pub const ALL: [Self; 6] = [
        Self::Notch,
        Self::Join,
        Self::Intersect,
        Self::Split,
        Self::Annular,
        Self::Pockets,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Self::Notch => "notch",
            Self::Join => "join",
            Self::Intersect => "intersect",
            Self::Split => "split",
            Self::Annular => "annular",
            Self::Pockets => "pockets",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Notch => "Sidewall notch",
            Self::Join => "Circle Join",
            Self::Intersect => "Circle Intersect",
            Self::Split => "Split cut — two bodies",
            Self::Annular => "Annular cut — two bodies",
            Self::Pockets => "Overlapping blind pockets",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.key() == key)
    }
    pub fn document(self) -> Result<Document> {
        let disk = |center, radius| Profile::Circle { center, radius };
        let rectangle = |a, b| Profile::Rectangle { a, b };
        let (outer, tool, holes, operation, combine) = match self {
            Self::Notch => (
                rectangle([-20., -10.], [20., 10.]),
                disk([0., -10.], 5.),
                vec![],
                Operation::Cut,
                false,
            ),
            Self::Join => (
                disk([-5., 0.], 10.),
                disk([5., 0.], 10.),
                vec![],
                Operation::Join,
                true,
            ),
            Self::Intersect => (
                disk([-5., 0.], 10.),
                disk([5., 0.], 10.),
                vec![],
                Operation::Intersect,
                true,
            ),
            Self::Split => (
                rectangle([-20., -10.], [20., 10.]),
                rectangle([-1., -15.], [1., 15.]),
                vec![],
                Operation::Cut,
                false,
            ),
            Self::Annular => (
                disk([0.; 2], 10.),
                disk([0.; 2], 8.),
                vec![disk([0.; 2], 3.)],
                Operation::Cut,
                false,
            ),
            Self::Pockets => (
                rectangle([-30.; 2], [30.; 2]),
                disk([-5., 0.], 10.),
                vec![],
                Operation::Cut,
                false,
            ),
        };
        let sketch =
            |id, name: &str, profile: Profile, holes: Vec<Profile>, z: f64| -> Result<Sketch> {
                let mut s = Sketch {
                    id,
                    name: name.into(),
                    origin: [0., 0., z],
                    ..Default::default()
                };
                s.add_profile(profile)?;
                for p in holes {
                    s.add_profile(p)?;
                }
                Ok(s)
            };
        let mut d = Document {
            name: format!("Rust engine — {}", self.title()),
            backend: Backend::Rust,
            ..Default::default()
        };
        d.sketches
            .push(sketch(1, "Base profile", outer, vec![], 0.)?);
        d.sketches.push(sketch(
            4,
            if self == Self::Pockets {
                "Pocket 1 profile"
            } else {
                "Tool profile"
            },
            tool,
            holes,
            if self == Self::Pockets { 10. } else { 0. },
        )?);
        d.extrusions.push(extrusion(
            2,
            1,
            3,
            if self == Self::Pockets { 10. } else { 5. },
            Operation::NewBody,
            "Base extrusion",
        ));
        d.extrusions.push(extrusion(
            5,
            4,
            if combine { 6 } else { 3 },
            if self == Self::Pockets { -3. } else { 5. },
            if combine {
                Operation::NewBody
            } else {
                operation
            },
            if self == Self::Pockets {
                "Pocket 1 — 3 mm"
            } else {
                "Tool extrusion"
            },
        ));
        if combine {
            d.push_feature(Feature {
                id: 7,
                name: self.title().into(),
                target: Some(3),
                outputs: vec![],
                kind: FeatureKind::Combine {
                    tool_body: 6,
                    operation,
                    keep_tool: false,
                },
            })?;
        }
        if self == Self::Pockets {
            d.sketches.push(sketch(
                7,
                "Pocket 2 profile",
                disk([5., 0.], 10.),
                vec![],
                10.,
            )?);
            d.extrusions
                .push(extrusion(8, 7, 3, -5., Operation::Cut, "Pocket 2 — 5 mm"));
        }
        d.validate()?;
        Ok(d)
    }
}
fn extrusion(
    id: u64,
    sketch_id: u64,
    body_id: u64,
    distance: f64,
    operation: Operation,
    name: &str,
) -> Extrusion {
    Extrusion {
        id,
        sketch_id,
        body_id,
        distance,
        operation,
        name: name.into(),
        profile: 0,
        region_id: Some(1),
        distance_expression: None,
        result_body_ids: vec![],
        boundary_dependencies: None,
        options: Default::default(),
    }
}
