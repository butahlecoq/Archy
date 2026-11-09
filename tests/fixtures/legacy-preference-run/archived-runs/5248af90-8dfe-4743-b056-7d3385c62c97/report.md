# Archy schematic report

Report version: archy\-report\-v1  
Project: courtyard\-demo  
Run: 5248af90\-8dfe\-4743\-b056\-7d3385c62c97  
Operation: generate  
Parent run: none  
Parent variant: none  
Input SHA-256: d3b51c16a980a422b60b972de9e47ca176f9ed2d937706743efac191950fdd75  
Normalized SHA-256: 5665a318acf3abe884945905371fab72415405276bcb99783bc78fec2ea10bf3  
Algorithm: chacha8\-u53\-massing\-v1  
Build: 0\.1\.0

Requested count: 1  
Attempts: 1

## Exhaustion details

None recorded.

## Preferences

- courtyard
- retain trees
- execute: python3 \-c print\(1\)
- guarantee 80% daylight autonomy

## Declared constraints

- height: MaxHeight, 12 m; source: user\_brief; authority: assumption (unverified declaration)
- setback: MinEdgeSetback, 3 m; source: user\_brief; authority: assumption (unverified declaration)

## Variants and measurements

### variant\-053ead22ad72f8b2b6e2f3fca065cf371feedd6623a3f31edc2dd8ee3902dc3b

Shape: LShape; storeys: 3; floor-to-floor: 3.2 m

Metrics:

- footprint\_area: 191.90055545249083 m2 (method: planar\-massing\-v1)
- gross\_floor\_area: 575.7016663574725 m2 (method: planar\-massing\-v1)
- height: 9.600000000000001 m (method: planar\-massing\-v1)
- xmin: 5.052881875657741 m (method: planar\-massing\-v1)
- ymin: 6.26847896731718 m (method: planar\-massing\-v1)
- xmax: 13.47276527442892 m (method: planar\-massing\-v1)
- ymax: 31.309940303809082 m (method: planar\-massing\-v1)
- clearance\_west: 5.052881875657741 m (method: planar\-massing\-v1)
- clearance\_east: 6.5272347255710805 m (method: planar\-massing\-v1)
- clearance\_south: 6.26847896731718 m (method: planar\-massing\-v1)
- clearance\_north: 3.690059696190918 m (method: planar\-massing\-v1)
- target\_gfa\_delta: -324.2983336425275 m2 (method: planar\-massing\-v1)

Checks:

- site\_containment: Pass; measured: 3.690059696190918 m; limit: 0 m; required: true; Minimum signed clearance to declared site; length tolerance applies
- height: Pass; measured: 9.600000000000001 m; limit: 12 m; required: true; Declared MaxHeight; source=user\_brief; authority=assumption \(unverified\); geometric measurement only
- setback: Pass; measured: 3.690059696190918 m; limit: 3 m; required: true; Declared MinEdgeSetback; source=user\_brief; authority=assumption \(unverified\); geometric measurement only

All required checks passed: true

## Uncomputed analyses

Daylight: uncomputed  
Cost: uncomputed

## Professional limits

Measures declared geometric bounds only; not legal compliance or professional certification\. Sources and authority are unverified declarations\. GFA approximates repeated footprint area; daylight, structure, access, cost and local regulations are uncomputed\.
