//! Saved/reopened command fixtures and exact neutral-format measurement checks.
use fission180_core::{
    Document, Extrusion, Model, Operation, Profile, Session, Sketch,
    features::{Axis, Feature, FeatureKind, RegionRef},
    kernel::Backend,
    persistence,
};
use std::path::PathBuf;
fn sketch(id: u64, z: f64, a: [f64; 2], b: [f64; 2]) -> Sketch {
    let mut s = Sketch {
        id,
        name: format!("Sketch{id}"),
        origin: [0., 0., z],
        ..Default::default()
    };
    s.add_profile(Profile::Rectangle { a, b }).unwrap();
    s
}
fn block(backend: Backend) -> Document {
    Document {
        backend,
        sketches: vec![sketch(1, 0., [0., 0.], [40., 30.])],
        extrusions: vec![Extrusion {
            id: 2,
            name: "Block".into(),
            sketch_id: 1,
            profile: 0,
            region_id: Some(1),
            distance: 5.,
            distance_expression: None,
            operation: Operation::NewBody,
            body_id: 3,
            result_body_ids: vec![],
            options: Default::default(),
        }],
        ..Default::default()
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an output directory")?,
    );
    std::fs::create_dir_all(&directory)?;
    let mut records = vec![];
    for backend in [Backend::Occt, Backend::Rust] {
        let mut cases = vec![
            (
                "hole",
                FeatureKind::Hole {
                    center: [20., 15.],
                    start_z: 0.,
                    diameter: "10 mm".into(),
                    depth: "5 mm".into(),
                },
            ),
            (
                "translate",
                FeatureKind::Translate {
                    offset: ["10".into(), "5".into(), "2".into()],
                    copy: false,
                },
            ),
            (
                "rectangular-pattern",
                FeatureKind::RectangularPattern {
                    axis: Axis::X,
                    spacing: "50".into(),
                    count: 3,
                },
            ),
            (
                "split-body",
                FeatureKind::SplitBody {
                    axis: Axis::X,
                    position: "20".into(),
                },
            ),
        ];
        if backend == Backend::Occt {
            cases.extend([
                ("fillet", FeatureKind::Fillet { radius: "1".into() }),
                (
                    "chamfer",
                    FeatureKind::Chamfer {
                        distance: "1".into(),
                    },
                ),
                (
                    "shell",
                    FeatureKind::Shell {
                        thickness: "1".into(),
                        open_axis: Axis::Z,
                        positive: true,
                    },
                ),
                (
                    "press-pull",
                    FeatureKind::OffsetFace {
                        distance: "2".into(),
                        axis: Axis::Z,
                        positive: true,
                    },
                ),
                (
                    "mirror",
                    FeatureKind::Mirror {
                        axis: Axis::X,
                        origin: [0.; 3],
                    },
                ),
                (
                    "circular-pattern",
                    FeatureKind::CircularPattern {
                        axis: Axis::Z,
                        origin: [-50., 0., 0.],
                        degrees: 360.,
                        count: 4,
                    },
                ),
                (
                    "revolve",
                    FeatureKind::Revolve {
                        region: RegionRef {
                            sketch_id: 1,
                            region_id: 1,
                        },
                        axis: Axis::Y,
                        axis_origin: [0.; 3],
                        degrees: 360.,
                    },
                ),
                (
                    "loft",
                    FeatureKind::Loft {
                        sections: vec![
                            RegionRef {
                                sketch_id: 1,
                                region_id: 1,
                            },
                            RegionRef {
                                sketch_id: 4,
                                region_id: 1,
                            },
                        ],
                        ruled: false,
                    },
                ),
                (
                    "sweep",
                    FeatureKind::Sweep {
                        region: RegionRef {
                            sketch_id: 1,
                            region_id: 1,
                        },
                        path_sketch: 4,
                        path_entities: vec![3],
                    },
                ),
            ]);
        }
        for (name, kind) in cases {
            let mut d = block(backend);
            if matches!(kind, FeatureKind::Revolve { .. }) {
                d.sketches = vec![sketch(1, 0., [5., 0.], [10., 20.])];
                d.extrusions.clear();
            }
            if matches!(kind, FeatureKind::Loft { .. }) {
                d.sketches.push(sketch(4, 20., [0., 0.], [40., 30.]));
                d.extrusions.clear();
            }
            if matches!(kind, FeatureKind::Sweep { .. }) {
                let mut path = Sketch {
                    id: 4,
                    name: "Path".into(),
                    plane: fission180_core::Plane::XZ,
                    ..Default::default()
                };
                path = path.with_line([0., 0.], [0., 20.], &d.parameters)?;
                d.sketches.push(path);
                d.extrusions.clear();
            }
            let target = if matches!(
                kind,
                FeatureKind::Revolve { .. } | FeatureKind::Loft { .. } | FeatureKind::Sweep { .. }
            ) {
                None
            } else {
                Some(3)
            };
            let id = d.next_id()?;
            d.push_feature(Feature {
                id,
                name: name.into(),
                target,
                outputs: vec![],
                kind,
            })?;
            d.name = format!("{name}-{}", backend.label());
            let session = Session::new(d)?;
            let path = directory.join(format!(
                "{name}-{}.ofrust",
                if backend == Backend::Rust {
                    "rust"
                } else {
                    "occt"
                }
            ));
            persistence::save(&session.document, &path)?;
            let reopened = Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
            assert_eq!(session.document, reopened.document);
            let info = reopened.model.info();
            let volume: f64 = info.iter().map(|b| b.volume).sum();
            let step_volume = if backend == Backend::Occt {
                let step = reopened.model.step_bytes()?;
                let imported = Model::step_volume(&step)?;
                assert!((volume - imported).abs() < 1e-4);
                std::fs::write(path.with_extension("step"), step)?;
                Some(imported)
            } else {
                None
            };
            records.push(serde_json::json!({"scenario":name,"backend":backend,"path":path,"saved_reopened":true,"step_reimport_volume_mm3":step_volume,"bodies":info.iter().map(|b|serde_json::json!({"id":b.id,"volume_mm3":b.volume,"area_mm2":b.surface_area,"bounds_mm":b.bounds})).collect::<Vec<_>>(),"display_triangles":reopened.model.display_mesh()?.indices.len()/3}));
        }
    }
    std::fs::write(
        directory.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!(
        "Saved and reopened {} command fixtures in {}",
        records.len(),
        directory.display()
    );
    Ok(())
}
