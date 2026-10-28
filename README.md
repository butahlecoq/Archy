# Archy

Archy is a proposed open-source system for agent-centered architectural design on Omarchy. It aims to turn a structured architectural brief into inspectable design alternatives, BIM deliverables, and reproducible comparisons.

This repository contains the initial project plan. No software, integrations, or automated workflow have been implemented yet.

Omarchy is the initial target environment for development and the proof of concept. The project should document any Omarchy-specific setup while keeping its architectural specifications independent of the desktop environment.

## Direction

Archy owns the project specification, constraints, design decisions, and evaluation history. Existing tools provide specialized modeling and BIM capabilities. The intended source of truth is a structured design specification together with IFC data; a Blender scene is a derived view, not the sole record of a design.

The first milestone is a proof of concept that takes one brief, generates three building masses without manual modeling, checks their stated geometric constraints, renders them, and produces a comparison whose measurements can be independently reproduced.

## Documents

- [Architecture](docs/architecture.md): proposed responsibilities, data, and boundaries.
- [Proof of concept](docs/proof-of-concept.md): workflow, deliverables, and success criteria.
- [Integrations](docs/integrations.md): candidate projects and questions to investigate.
- [Project structure](docs/project-structure.md): proposed layout for an architectural project managed in Git.
- [Roadmap](docs/roadmap.md): milestones and decisions still to make.

Archy is a working name. Trademark, package, and repository-name availability need further investigation. A license has not yet been chosen.
