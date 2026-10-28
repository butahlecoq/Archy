# Proposed architectural project structure

An Archy-managed design project could keep its specification and deliverables in Git. This is an illustrative layout, not an implemented file format.

```text
housing-project/
├── archy.yaml                    # Project metadata and references
├── site/
│   └── boundary.geojson          # Site boundary
├── design/
│   ├── massing.yaml              # Alternative massing parameters
│   └── floorplans.yaml           # Future floor plan specification
├── constraints/
│   └── requirements.yaml        # Area and geometric requirements
├── models/
│   └── building.ifc              # BIM model
├── renders/                      # Generated views
└── reports/                      # Measurements and comparisons
```

The schemas, coordinate system, units, artifact naming, and versioning rules are open decisions. The first proof of concept needs only the fields and artifacts required for its massing study. Whether generated IFC files and renders belong in Git, external storage, or both also remains open.

A future agent workflow may read the requirements, change design parameters, generate the model, evaluate alternatives, and propose a reviewable revision. Committing changes is a separate operation that requires an agreed workflow.
