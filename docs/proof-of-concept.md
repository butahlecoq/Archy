# Archy 0.1 proof of concept

The proof of concept tests whether an existing agent and modeling tools can carry an architectural brief through a reproducible massing study.

## Proposed workflow

1. Connect an existing coding agent to Blender MCP and IfcMCP.
2. Read a machine-readable brief with site dimensions, floor count, setbacks, and area requirements.
3. Use a small set of deterministic architectural operations to generate three alternative building masses.
4. Calculate floor area and evaluate each alternative against the specified geometric constraints.
5. Render the alternatives and produce a comparison report.

## Expected outputs

Each alternative needs recorded design parameters, a model with relevant BIM semantics, a render, and evaluation results. The comparison report needs the input brief, measurement definitions and units, constraint outcomes, and enough information to reproduce each reported measurement independently. The IFC generation and verification approach remains to be established during the proof of concept.

## Success criteria

Starting from one brief, the agent completes the workflow without manual modeling. All reported measurements can be independently reproduced. The three alternatives and their constraint results can be inspected and compared.

This milestone evaluates the constraints stated in the brief. It does not establish building-code compliance or validate circulation, detailed floor plans, or construction readiness.
