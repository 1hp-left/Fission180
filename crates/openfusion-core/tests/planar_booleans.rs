use fission180_core::{
    Profile,
    arc::ArcGeometry,
    boundary::{BoundarySegment, PlanarCurve},
    planar::{Boolean, Region, RegionSet},
};
use std::f64::consts::{PI, TAU};
fn disk(x: f64, r: f64) -> Profile {
    Profile::Circle {
        center: [x, 0.],
        radius: r,
    }
}
fn rectangle(x: f64, y: f64, w: f64, h: f64) -> Profile {
    Profile::Rectangle {
        a: [x, y],
        b: [x + w, y + h],
    }
}
fn set(p: Profile, holes: Vec<Profile>, feature: u64) -> RegionSet {
    let ids = (1..=holes.len() as u64 + 1).collect::<Vec<_>>();
    RegionSet {
        regions: vec![Region::from_profiles(&p, &holes, feature, 1, &ids).unwrap()],
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}");
}
#[test]
fn circles_analytic_lens_conservation_and_commutativity() {
    let a = set(disk(0., 10.), vec![], 1);
    let b = set(disk(10., 10.), vec![], 2);
    let lens = 200. * PI / 3. - 50. * 3f64.sqrt();
    for (op, expected) in [
        (Boolean::Intersection, lens),
        (Boolean::Union, 200. * PI - lens),
        (Boolean::Difference, 100. * PI - lens),
    ] {
        let r = a.boolean(&b, op).unwrap();
        r.validate().unwrap();
        close(r.area(), expected);
        assert_eq!(r.regions.len(), 1);
        assert!(
            r.regions[0]
                .outer
                .spans
                .iter()
                .all(|s| matches!(s.curve, PlanarCurve::Arc { .. }))
        );
        assert!(
            r.regions[0]
                .outer
                .spans
                .iter()
                .flat_map(|s| &s.ancestry)
                .any(|s| (s.interval[1] - s.interval[0]).abs() < 0.99)
        );
        if op != Boolean::Difference {
            close(b.boolean(&a, op).unwrap().area(), expected);
        }
    }
    close(
        a.boolean(&b, Boolean::Union).unwrap().area() + lens,
        200. * PI,
    );
}
#[test]
fn shared_line_spans_identical_regions_and_equivalent_circle_segmentation() {
    let a = set(rectangle(0., 0., 20., 10.), vec![], 1);
    let b = set(rectangle(10., 0., 20., 10.), vec![], 2);
    for (op, expected) in [
        (Boolean::Union, 300.),
        (Boolean::Intersection, 100.),
        (Boolean::Difference, 100.),
    ] {
        close(a.boolean(&b, op).unwrap().area(), expected);
    }
    let segmented = Profile::Curved {
        segments: (0..7)
            .rev()
            .map(|i| BoundarySegment {
                source: Some(100 + i),
                curve: PlanarCurve::Arc {
                    geometry: ArcGeometry {
                        center: [0.; 2],
                        radius: 10.,
                        start_angle: (i + 1) as f64 * TAU / 7.,
                        sweep: -TAU / 7.,
                    },
                },
            })
            .collect(),
    };
    let a = set(disk(0., 10.), vec![], 3);
    let b = set(segmented, vec![], 4);
    assert!(
        a.boolean(&b, Boolean::Difference)
            .unwrap()
            .regions
            .is_empty()
    );
    for op in [Boolean::Union, Boolean::Intersection] {
        let r = a.boolean(&b, op).unwrap();
        close(r.area(), 100. * PI);
        assert!(
            r.regions[0]
                .outer
                .spans
                .iter()
                .all(|s| s.ancestry.len() == 2)
        );
    }
    for a in [a, set(rectangle(0., 0., 20., 10.), vec![], 5)] {
        assert!(
            a.boolean(&a, Boolean::Difference)
                .unwrap()
                .regions
                .is_empty()
        );
        for op in [Boolean::Union, Boolean::Intersection] {
            close(a.boolean(&a, op).unwrap().area(), a.area());
        }
    }
}
#[test]
fn holes_nested_islands_annular_cut_and_disconnected_strip() {
    let a = set(disk(0., 10.), vec![], 1);
    let b = set(disk(0., 8.), vec![disk(0., 3.)], 2);
    let r = a.boolean(&b, Boolean::Difference).unwrap();
    assert_eq!(r.regions.len(), 2);
    close(r.area(), 45. * PI);
    assert_eq!(r.regions.iter().map(|r| r.holes.len()).sum::<usize>(), 1);
    let a = set(rectangle(0., 0., 40., 20.), vec![], 3);
    let b = set(rectangle(19., -5., 2., 30.), vec![], 4);
    let r = a.boolean(&b, Boolean::Difference).unwrap();
    assert_eq!(r.regions.len(), 2);
    for r in &r.regions {
        close(r.area(), 380.);
    }
    let outside = set(disk(100., 2.), vec![], 6);
    assert!(
        a.boolean(&outside, Boolean::Intersection)
            .unwrap()
            .regions
            .is_empty()
    );
    assert_eq!(
        a.boolean(&outside, Boolean::Union).unwrap().regions.len(),
        2
    );
    let cover = set(rectangle(-10., -10., 100., 100.), vec![], 7);
    assert!(
        a.boolean(&cover, Boolean::Difference)
            .unwrap()
            .regions
            .is_empty()
    );
}
#[test]
fn tangent_contacts_resolution_malformed_and_resource_limits() {
    let a = set(disk(0., 10.), vec![], 1);
    let b = set(disk(20., 10.), vec![], 2);
    assert!(
        a.boolean(&b, Boolean::Intersection)
            .unwrap()
            .regions
            .is_empty()
    );
    assert!(
        a.boolean(&b, Boolean::Union)
            .unwrap_err()
            .to_string()
            .contains("contact")
    );
    close(
        a.boolean(&b, Boolean::Difference).unwrap().area(),
        100. * PI,
    );
    for x in [20. - 1e-6, 20. + 1e-6, 1e-6] {
        assert!(
            a.boolean(&set(disk(x, 10.), vec![], 3), Boolean::Union)
                .is_err()
        );
    }
    let mut bad = a.clone();
    bad.regions[0].outer.spans[0].curve = PlanarCurve::Line {
        start: [f64::NAN, 0.],
        end: [1., 0.],
    };
    assert!(bad.validate().is_err());
    let mut bad = a.clone();
    bad.regions[0].outer.spans[0].ancestry[0].interval = [0., f64::INFINITY];
    assert!(bad.validate().is_err());
    let mut huge = a.clone();
    huge.regions = vec![a.regions[0].clone(); 300];
    assert!(huge.validate().unwrap_err().to_string().contains("512"));
    let malformed = set(
        rectangle(-20., -20., 40., 40.),
        vec![disk(0., 5.), disk(2., 5.)],
        6,
    );
    assert!(malformed.validate().is_err());
}
#[test]
fn mixed_boundaries_sidewall_half_disk_and_source_stability() {
    let a = set(rectangle(0., 0., 40., 20.), vec![], 1);
    let b = set(
        Profile::Circle {
            center: [20., 0.],
            radius: 5.,
        },
        vec![],
        2,
    );
    let r = a.boolean(&b, Boolean::Difference).unwrap();
    close(r.area(), 800. - 12.5 * PI);
    assert_eq!(r.regions.len(), 1);
    assert!(
        r.regions[0]
            .outer
            .spans
            .iter()
            .any(|s| matches!(s.curve, PlanarCurve::Line { .. }))
    );
    assert!(
        r.regions[0]
            .outer
            .spans
            .iter()
            .any(|s| matches!(s.curve, PlanarCurve::Arc { .. }))
    );
    let a = set(disk(0., 10.), vec![], 3);
    let b = set(disk(10., 10.), vec![], 4);
    let r = a.boolean(&b, Boolean::Intersection).unwrap();
    let b = set(disk(11., 10.), vec![], 4);
    let changed = a.boolean(&b, Boolean::Intersection).unwrap();
    assert_eq!(r.regions[0].key(), changed.regions[0].key());
}
