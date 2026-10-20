use fission180_core::{
    Document, Extrusion, Operation, Plane, Profile, Sketch,
    kernel::{Backend, ExtrudeInput},
};
use std::f64::consts::PI;
pub fn input(outer: Profile, holes: Vec<Profile>, feature_id: u64) -> ExtrudeInput {
    ExtrudeInput {
        feature_id,
        region_id: 1,
        loop_ids: (1..=holes.len() as u64 + 1).collect(),
        outer,
        holes,
        plane: Plane::XY,
        origin: [0.; 3],
        distance: 5.,
    }
}
pub fn disk(x: f64, r: f64) -> Profile {
    Profile::Circle {
        center: [x, 0.],
        radius: r,
    }
}
pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Profile {
    Profile::Rectangle {
        a: [x, y],
        b: [x + w, y + h],
    }
}
pub struct Case {
    pub name: &'static str,
    pub a: ExtrudeInput,
    pub b: ExtrudeInput,
    pub op: Operation,
    pub areas: Vec<f64>,
    pub perimeter: f64,
}
pub fn cases() -> Vec<Case> {
    let mut v = vec![];
    let lens = 200. * PI / 3. - 50. * 3f64.sqrt();
    for (name, op, area, perimeter) in [
        ("lens", Operation::Intersect, lens, 40. * PI / 3.),
        (
            "circle-union",
            Operation::Join,
            200. * PI - lens,
            80. * PI / 3.,
        ),
        (
            "circle-difference",
            Operation::Cut,
            100. * PI - lens,
            20. * PI,
        ),
    ] {
        v.push(Case {
            name,
            a: input(disk(0., 10.), vec![], 2),
            b: input(disk(10., 10.), vec![], 5),
            op,
            areas: vec![area],
            perimeter,
        });
    }
    v.push(Case {
        name: "notch",
        a: input(rect(0., 0., 40., 20.), vec![], 2),
        b: input(
            Profile::Circle {
                center: [20., 0.],
                radius: 5.,
            },
            vec![],
            5,
        ),
        op: Operation::Cut,
        areas: vec![800. - 12.5 * PI],
        perimeter: 110. + 5. * PI,
    });
    v.push(Case {
        name: "strip-split",
        a: input(rect(0., 0., 40., 20.), vec![], 2),
        b: input(rect(19., -5., 2., 30.), vec![], 5),
        op: Operation::Cut,
        areas: vec![380., 380.],
        perimeter: 156.,
    });
    v.push(Case {
        name: "annular-split",
        a: input(disk(0., 10.), vec![], 2),
        b: input(disk(0., 8.), vec![disk(0., 3.)], 5),
        op: Operation::Cut,
        areas: vec![9. * PI, 36. * PI],
        perimeter: 42. * PI,
    });
    v.push(Case {
        name: "disjoint-join",
        a: input(disk(0., 10.), vec![], 2),
        b: input(disk(30., 2.), vec![], 5),
        op: Operation::Join,
        areas: vec![100. * PI, 4. * PI],
        perimeter: 24. * PI,
    });
    v.push(Case {
        name: "fully-covered",
        a: input(disk(0., 10.), vec![], 2),
        b: input(rect(-20., -20., 40., 40.), vec![], 5),
        op: Operation::Cut,
        areas: vec![],
        perimeter: 0.,
    });
    v
}
pub fn extrusion(
    id: u64,
    sketch: u64,
    body: u64,
    distance: f64,
    operation: Operation,
) -> Extrusion {
    Extrusion {
        id,
        name: format!("Feature{id}"),
        sketch_id: sketch,
        profile: 0,
        region_id: Some(1),
        distance,
        operation,
        body_id: body,
        distance_expression: None,
        result_body_ids: vec![],
        boundary_dependencies: None,
        options: Default::default(),
    }
}
pub fn document(
    a: &ExtrudeInput,
    b: &ExtrudeInput,
    op: Operation,
    backend: Backend,
    combine: bool,
) -> Document {
    let sketch = |id, input: &ExtrudeInput| {
        let mut s = Sketch {
            id,
            plane: input.plane,
            origin: input.origin,
            ..Default::default()
        };
        s.add_profile(input.outer.clone()).unwrap();
        for h in &input.holes {
            s.add_profile(h.clone()).unwrap();
        }
        s
    };
    let mut d = Document {
        backend,
        sketches: vec![sketch(1, a), sketch(4, b)],
        extrusions: vec![
            extrusion(2, 1, 3, a.distance, Operation::NewBody),
            extrusion(
                5,
                4,
                if combine { 6 } else { 3 },
                b.distance,
                if combine { Operation::NewBody } else { op },
            ),
        ],
        ..Default::default()
    };
    if combine {
        d.push_feature(fission180_core::features::Feature {
            id: 7,
            name: "Combine".into(),
            target: Some(3),
            outputs: vec![],
            kind: fission180_core::features::FeatureKind::Combine {
                tool_body: 6,
                operation: op,
                keep_tool: false,
            },
        })
        .unwrap();
    }
    d
}
