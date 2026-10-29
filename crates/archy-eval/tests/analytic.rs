use archy_core::*;
use archy_eval::evaluate_variant;
fn spec() -> NormalizedSpec {
    parse_project(include_str!("../../../tests/fixtures/briefs/courtyard.json"), InputFormat::Json).unwrap()
}
fn rectangle(s: &NormalizedSpec, origin: [f64; 2]) -> Variant {
    Variant::new(s.project_id.clone(), 42, "analytic-v1".into(), ShapeParameters::Rectangle {origin_xy_m: origin, width_m: 10., depth_m: 20.},3,3.2).unwrap()
}
fn metric(r: &EvaluationReport, name: &str) -> f64 {r.metrics.iter().find(|m| m.name == name).unwrap().value}
#[test]
fn rectangle_and_l_have_independent_analytic_measurements_and_translation() {
    let mut s = spec();
    let v = rectangle(&s,[3.,3.]);
    let r = evaluate_variant(&s,&v).unwrap();
    for (name,value) in [("footprint_area",200.),("gross_floor_area",600.),("height",9.6),("clearance_west",3.),("clearance_east",7.),("clearance_south",3.),("clearance_north",12.)] { assert!((metric(&r,name)-value).abs() < 1e-10); }
    let l = Variant::new(s.project_id.clone(),42,"analytic-v1".into(),ShapeParameters::LShape {origin_xy_m:[3.,3.],width_m:10.,depth_m:10.,notch_width_m:5.,notch_depth_m:5.},3,3.2).unwrap();
    assert_eq!(metric(&evaluate_variant(&s,&l).unwrap(),"footprint_area"),75.);
    assert_eq!(metric(&evaluate_variant(&s,&l).unwrap(),"gross_floor_area"),225.);
    for p in &mut s.site.polygon_xy_m {p[0]+=10.;p[1]-=5.;}
    let translated = evaluate_variant(&s,&rectangle(&s,[13.,-2.])).unwrap();
    for name in ["footprint_area","gross_floor_area","height","clearance_west","clearance_east","clearance_south","clearance_north"] {assert!((metric(&r,name)-metric(&translated,name)).abs()<1e-10);}
}
#[test]
fn canonical_snapshot_rejects_all_tampering_and_malformed_geometry() {
    let s=spec(); let v=rectangle(&s,[3.,3.]);
    for field in ["id","project_id","generation_algorithm"] {
        let mut value=serde_json::to_value(&v).unwrap();value[field]=serde_json::json!("tampered");
        assert!(serde_json::from_value::<Variant>(value).is_err());
    }
    let mut value=serde_json::to_value(&v).unwrap(); value["parameters"]["width_m"]=serde_json::json!(11.);
    assert!(serde_json::from_value::<Variant>(value).is_err());
    let mut bads=vec![];
    let mut b=v.clone();b.footprint_xy_m.reverse();bads.push(b);
    let mut b=v.clone();b.footprint_xy_m.insert(1,b.footprint_xy_m[0]);bads.push(b);
    let mut b=v.clone();b.footprint_xy_m.swap(1,2);bads.push(b);
    let mut b=v.clone();b.footprint_xy_m.pop();bads.push(b);
    let mut b=v.clone();b.footprint_xy_m[1][0]=f64::NAN;bads.push(b);
    let mut b=v.clone();b.footprint_xy_m.fill([3.,3.]);bads.push(b);
    for b in bads { assert_eq!(evaluate_variant(&s,&b).unwrap_err().code, ErrorCode::InvalidGeometry); }
    let mut other=s.clone();other.project_id="other".into(); assert!(evaluate_variant(&other,&v).is_err());
    assert!(Variant::new(s.project_id,42,"analytic-v1".into(),ShapeParameters::Rectangle{origin_xy_m:[0.,0.],width_m:f64::MAX,depth_m:f64::MAX},3,3.2).is_err());
}
#[test]
fn tolerance_hard_target_outside_and_authority_are_explicit() {
    let mut s=spec();
    for (delta,pass) in [(0.,true),(-0.5e-6,true),(-2e-6,false)] {
        let r=evaluate_variant(&s,&rectangle(&s,[3.+delta,3.])).unwrap();assert_eq!(r.required_checks_passed,pass);
    }
    for (delta,pass) in [(0.,true),(0.5e-6,true),(2e-6,false)] {
        s.constraints[0].value_m=9.6-delta;
        assert_eq!(evaluate_variant(&s,&rectangle(&s,[3.,3.])).unwrap().required_checks_passed,pass);
    }
    s=spec();s.brief.target_gfa_hard=true;
    for (delta,pass) in [(0.,true),(0.5e-6,true),(2e-6,false)] {
        s.brief.target_gfa_m2=600.+delta;
        assert_eq!(evaluate_variant(&s,&rectangle(&s,[3.,3.])).unwrap().required_checks_passed,pass);
    }
    s=spec();let r=evaluate_variant(&s,&rectangle(&s,[-2.,3.])).unwrap();assert_eq!(metric(&r,"clearance_west"),-2.);assert!(!r.required_checks_passed);
    s.constraints[0].authority="CERTIFIED LEGAL LAW".into();
    let r=evaluate_variant(&s,&rectangle(&s,[3.,3.])).unwrap();assert!(r.professional_limits.contains("not legal"));
    let mut checks=r.checks; checks.push(Check{constraint_id:"uncomputed".into(),result:CheckResult::Unknown,measured:None,limit:None,explanation:"Uncomputed".into(),required:true});
    assert!(!required_checks_pass(&checks));
}
