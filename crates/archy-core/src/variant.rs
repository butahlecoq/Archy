//! Canonical parameters are authoritative. Stored footprints and identities are checked.
use crate::{ArchyError, ErrorCode, NormalizedSpec, Shape, units::Units};
use geo::{Area, BoundingRect, LineString, Polygon, Validation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MIN_DIMENSION_M: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShapeParameters {
    Rectangle { origin_xy_m: [f64;2], width_m: f64, depth_m: f64 },
    LShape { origin_xy_m: [f64;2], width_m: f64, depth_m: f64, notch_width_m: f64, notch_depth_m: f64 },
}
fn bad(field: &str, message: &str) -> ArchyError {
    ArchyError::new(ErrorCode::InvalidGeometry, field, message)
}
impl ShapeParameters {
    pub fn shape(&self) -> Shape { match self { Self::Rectangle{..} => Shape::Rectangle, Self::LShape{..} => Shape::LShape } }
    /// CCW, lower-left start; L removes a positive upper-right notch.
    pub fn footprint(&self) -> Result<Vec<[f64;2]>, ArchyError> {
        let (origin,width,depth,notch) = match *self {
            Self::Rectangle{origin_xy_m,width_m,depth_m} => (origin_xy_m,width_m,depth_m,None),
            Self::LShape{origin_xy_m,width_m,depth_m,notch_width_m,notch_depth_m} => (origin_xy_m,width_m,depth_m,Some((notch_width_m,notch_depth_m))),
        };
        if origin.iter().any(|v| !v.is_finite()) || [width,depth].iter().any(|v| !v.is_finite() || *v <= MIN_DIMENSION_M) {
            return Err(bad("parameters","Finite origin and dimensions above 1e-6m required"));
        }
        let [x,y] = origin;
        let (right,top) = (x+width,y+depth);
        let mut points = vec![[x,y],[right,y]];
        if let Some((nw,nd)) = notch {
            if [nw,nd,width-nw,depth-nd].iter().any(|v| !v.is_finite() || *v <= MIN_DIMENSION_M) {
                return Err(bad("parameters","Notch and remaining legs must exceed 1e-6m"));
            }
            points.extend([[right,top-nd],[right-nw,top-nd],[right-nw,top]]);
        } else { points.push([right,top]); }
        points.extend([[x,top],[x,y]]);
        // Reject precision collapse at translated origins, including very short legs.
        if points.windows(2).any(|p| {
            let length=(p[1][0]-p[0][0]).abs()+(p[1][1]-p[0][1]).abs();
            !length.is_finite() || length <= MIN_DIMENSION_M
        }) { return Err(bad("parameters","Coordinates below supported numeric resolution")); }
        validated_polygon(&points)?;
        Ok(points)
    }
    /// Geometric identity omits sampling metadata; canonical derivation fixes start/winding.
    pub fn geometric_signature(&self) -> Result<String, ArchyError> {
        let mut hash=Sha256::new(); hash.update(b"archy-footprint-v1");
        for point in self.footprint()? {for v in point {hash.update(canonical_bits(v));}}
        Ok(hex(&hash.finalize()))
    }
}

pub fn validated_polygon(points: &[[f64;2]]) -> Result<Polygon<f64>, ArchyError> {
    if points.len()<5 || points.first()!=points.last() || points.iter().flatten().any(|v| !v.is_finite()) || points.windows(2).any(|p|p[0]==p[1]) {
        return Err(bad("footprint_xy_m","Expected finite, explicitly closed polygon without consecutive duplicates"));
    }
    let polygon=Polygon::new(LineString::from(points.to_vec()),vec![]);
    polygon.check_validation().map_err(|_| bad("footprint_xy_m","Invalid polygon topology"))?;
    let area=polygon.unsigned_area();
    if !area.is_finite() || area <= 0. {return Err(bad("footprint_xy_m","Area must be positive and finite"));}
    Ok(polygon)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from="VariantData", deny_unknown_fields)]
pub struct Variant {
    pub schema_version: String,
    pub project_id: String,
    pub id: String,
    pub seed: u64,
    pub generation_algorithm: String,
    pub shape: Shape,
    pub parameters: ShapeParameters,
    pub footprint_xy_m: Vec<[f64;2]>,
    pub storeys: u32,
    pub floor_to_floor_m: f64,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Variant")]
struct VariantData {
    schema_version: String, project_id: String, id: String, seed: u64,
    generation_algorithm: String, shape: Shape, parameters: ShapeParameters,
    footprint_xy_m: Vec<[f64;2]>, storeys:u32, floor_to_floor_m:f64,
}
impl TryFrom<VariantData> for Variant {
    type Error=ArchyError;
    fn try_from(v:VariantData)->Result<Self,Self::Error> {
        let variant=Self{schema_version:v.schema_version,project_id:v.project_id,id:v.id,seed:v.seed,generation_algorithm:v.generation_algorithm,shape:v.shape,parameters:v.parameters,footprint_xy_m:v.footprint_xy_m,storeys:v.storeys,floor_to_floor_m:v.floor_to_floor_m};
        variant.validate()?; Ok(variant)
    }
}
fn canonical_bits(v:f64)->[u8;8] {if v==0. {0_f64.to_be_bytes()} else {v.to_be_bytes()}}
fn hex(bytes:&[u8])->String {bytes.iter().map(|b|format!("{b:02x}")).collect()}
fn string(hash:&mut Sha256,s:&str) {hash.update((s.len() as u64).to_be_bytes());hash.update(s.as_bytes());}
impl Variant {
    pub fn new(project_id:String,seed:u64,generation_algorithm:String,parameters:ShapeParameters,storeys:u32,floor_to_floor_m:f64)->Result<Self,ArchyError> {
        let footprint_xy_m=parameters.footprint()?;
        let mut v=Self{schema_version:"0.1.0".into(),project_id,id:String::new(),seed,generation_algorithm,shape:parameters.shape(),parameters,footprint_xy_m,storeys,floor_to_floor_m};
        v.id=v.canonical_id();v.validate()?;Ok(v)
    }
    fn canonical_id(&self)->String {
        let mut h=Sha256::new();h.update(b"archy-variant-v1");
        for s in [&self.schema_version,&self.project_id,&self.generation_algorithm] {string(&mut h,s);}
        h.update(self.seed.to_be_bytes());h.update(self.storeys.to_be_bytes());h.update(canonical_bits(self.floor_to_floor_m));
        h.update([match self.shape {Shape::Rectangle=>0,Shape::LShape=>1}]);
        // Explicit parameter order is stable across serde and compiler versions.
        let (origin,w,d,n)=match self.parameters {ShapeParameters::Rectangle{origin_xy_m,width_m,depth_m}=>(origin_xy_m,width_m,depth_m,None),ShapeParameters::LShape{origin_xy_m,width_m,depth_m,notch_width_m,notch_depth_m}=>(origin_xy_m,width_m,depth_m,Some((notch_width_m,notch_depth_m)))};
        for v in [origin[0],origin[1],w,d] {h.update(canonical_bits(v));}
        if let Some((w,d))=n {h.update(canonical_bits(w));h.update(canonical_bits(d));}
        format!("variant-{}",hex(&h.finalize()))
    }
    pub fn validate(&self)->Result<(),ArchyError> {
        if self.schema_version!="0.1.0" || self.project_id.is_empty() || self.generation_algorithm.is_empty() {return Err(ArchyError::schema("variant","Missing identity fields or unsupported schema"));}
        if self.storeys==0 || !self.floor_to_floor_m.is_finite() || self.floor_to_floor_m<=0. || !(self.floor_to_floor_m*f64::from(self.storeys)).is_finite() {return Err(bad("storeys","Storey height must be positive and finite"));}
        let expected=self.parameters.footprint()?;
        if self.shape!=self.parameters.shape() || self.footprint_xy_m!=expected {return Err(bad("footprint_xy_m","Stored geometry differs from canonical parameters/order"));}
        let area=validated_polygon(&expected)?.unsigned_area();
        if !(area*f64::from(self.storeys)).is_finite() {return Err(bad("storeys","Gross floor area overflow"));}
        if self.id!=self.canonical_id() {return Err(bad("id","Identifier differs from canonical parameter hash"));}
        Ok(())
    }
}

/// Validate public normalized geometry without reparsing or renormalizing it.
/// Buildable dimensions are deliberately left to bounded generation/exhaustion.
pub fn site_bounds(spec:&NormalizedSpec)->Result<[f64;4],ArchyError> {
    if spec.schema_version!="0.1.0" || spec.normalization_version!="si-rectangle-v1" || spec.units!=Units::M || spec.project_id.is_empty() || spec.tolerance_m!=crate::validation::TOLERANCE_M {
        return Err(ArchyError::schema("spec","Expected supported normalized SI project"));
    }
    let polygon=validated_polygon(&spec.site.polygon_xy_m)?;
    let b=polygon.bounding_rect().ok_or_else(||bad("site","Missing bounds"))?;
    let bounds=[b.min().x,b.min().y,b.max().x,b.max().y];
    let [x,y,r,t]=bounds;
    if spec.site.polygon_xy_m!=[[x,y],[r,y],[r,t],[x,t],[x,y]] || !(r-x).is_finite() || !(t-y).is_finite() || !spec.site.north_angle_deg.is_finite() {
        return Err(bad("site","Expected canonical finite rectangular site"));
    }
    let brief=&spec.brief;
    if brief.storeys==0 || [brief.floor_to_floor_m,brief.target_gfa_m2,brief.floor_to_floor_m*f64::from(brief.storeys)].iter().any(|v|!v.is_finite() || *v<=0.) {
        return Err(ArchyError::schema("brief","Expected finite positive brief dimensions"));
    }
    if spec.constraints.iter().any(|c|!c.value_m.is_finite() || c.value_m<=0.) {return Err(ArchyError::schema("constraints","Expected finite positive constraint limits"));}
    Ok(bounds)
}
