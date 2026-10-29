//! Measurements of declared schematic geometry.
use archy_core::*;
use archy_core::validation::{TOLERANCE_M, TOLERANCE_M2};
use geo::{Area, BoundingRect};

pub const EVALUATION_VERSION: &str = "planar-massing-v1";
pub const PROFESSIONAL_LIMITS: &str = "Measures declared geometric bounds only; not legal compliance or professional certification. Sources and authority are unverified declarations. GFA approximates repeated footprint area; daylight, structure, access, cost and local regulations are uncomputed.";

pub fn evaluate_variant(spec: &NormalizedSpec, variant: &Variant) -> Result<EvaluationReport, ArchyError> {
    variant.validate()?;
    let [sx,sy,sr,st]=site_bounds(spec)?;
    if variant.project_id!=spec.project_id {return Err(ArchyError::schema("project_id","Variant and project identifiers differ"));}
    let polygon=validated_polygon(&variant.footprint_xy_m)?;
    let bounds=polygon.bounding_rect().ok_or_else(||ArchyError::new(ErrorCode::InvalidGeometry,"footprint_xy_m","Missing bounds"))?;
    let (x,y,r,t)=(bounds.min().x,bounds.min().y,bounds.max().x,bounds.max().y);
    let area=polygon.unsigned_area();
    let gfa=area*f64::from(variant.storeys);
    let height=variant.floor_to_floor_m*f64::from(variant.storeys);
    let clearances=[x-sx,sr-r,y-sy,st-t];
    let minimum=clearances.iter().copied().fold(f64::INFINITY,f64::min);
    let mut metrics=Vec::new();
    for (name,value,unit) in [
        ("footprint_area",area,"m2"),("gross_floor_area",gfa,"m2"),("height",height,"m"),
        ("xmin",x,"m"),("ymin",y,"m"),("xmax",r,"m"),("ymax",t,"m"),
        ("clearance_west",clearances[0],"m"),("clearance_east",clearances[1],"m"),("clearance_south",clearances[2],"m"),("clearance_north",clearances[3],"m"),
        ("target_gfa_delta",gfa-spec.brief.target_gfa_m2,"m2")
    ] {
        if !value.is_finite() {return Err(ArchyError::new(ErrorCode::InvalidGeometry,name,"Measurement arithmetic overflow"));}
        metrics.push(Metric{name:name.into(),value,unit:unit.into(),method_version:EVALUATION_VERSION.into()});
    }
    let make=|id:&str,measured:f64,limit:f64,pass:bool,explanation:String|Check{constraint_id:id.into(),result:if pass{CheckResult::Pass}else{CheckResult::Fail},measured:Some(measured),limit:Some(limit),explanation,required:true};
    let mut checks=vec![make("site_containment",minimum,0.,minimum>=-TOLERANCE_M,"Minimum signed clearance to declared site; length tolerance applies".into())];
    for constraint in &spec.constraints {
        let (measured,pass)=match constraint.kind {
            ConstraintKind::MaxHeight=>(height,height-constraint.value_m<=TOLERANCE_M),
            ConstraintKind::MinEdgeSetback=>(minimum,constraint.value_m-minimum<=TOLERANCE_M),
        };
        checks.push(make(&constraint.id,measured,constraint.value_m,pass,format!("Declared {:?}; source={}; authority={} (unverified); geometric measurement only",constraint.kind,constraint.source,constraint.authority)));
    }
    if spec.brief.target_gfa_hard {
        checks.push(make("target_gfa",gfa,spec.brief.target_gfa_m2,spec.brief.target_gfa_m2-gfa<=TOLERANCE_M2,"Hard target interpreted as minimum approximate GFA; area tolerance applies".into()));
    }
    let required_checks_passed=required_checks_pass(&checks);
    Ok(EvaluationReport{variant_id:variant.id.clone(),evaluation_version:EVALUATION_VERSION.into(),metrics,checks,tolerance:DimensionalTolerance{length_m:TOLERANCE_M,area_m2:TOLERANCE_M2,method_version:"absolute-si-v1".into()},required_checks_passed,professional_limits:PROFESSIONAL_LIMITS.into()})
}
