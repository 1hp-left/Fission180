# Extrude invalid-input scenario

Local observation ID: `RUNTIME-002`. This is Computer Use evidence, not a REA Evidence record or a native call trace. Date: 2026-10-18. Fusion 2705.1.30, existing unsaved scratch Part Design, dark blue theme, millimetres. The captured main window was 1202 × 631 logical pixels in this pass; do not compare it directly to the earlier maximized 1536 × 864 reference.

## Initial state and actions

The scratch design contained the previously drawn 40 × 20 mm rectangle on XY and was still editing the sketch. No saved user design was opened or edited.

1. Finish Sketch returned to the Solid ribbon. The view remained Top in this scenario; automatic isometric reorientation must not be assumed for every sketch-exit path.
2. Press E. Extrude selected one profile; the status displayed area 800.00 mm². The HUD distance field held focus with `0.00 mm` selected. The panel had Profile Plane start, One Side direction, Distance extent, zero taper and New Body operation. OK was disabled at zero distance.
3. Type `5 mm`, then Tab. The panel showed `5 mm` and OK became enabled. Focus moved to the HUD taper-angle field. No extrusion was committed.
4. Shift+Tab selected `5 mm` in the HUD distance field. Replace it with the incomplete expression `5 +`.
5. The HUD retained the incomplete draft. The linked panel retained `5 mm`; a red error underline appeared in the distance row. The captured OK button still appeared enabled at this instant. No click on OK or delayed-state check was performed, so invalid-draft commit behavior remains unknown.
6. Press Escape once. The HUD and Extrude panel closed, the Solid ribbon remained, and the rectangle sketch was still visible. The Browser showed no Bodies group in the captured result. This is command cancellation in this scenario, not evidence of a field-only Escape reset.

## Retained artifacts

Files are outside the source tree under `C:\Users\Vnm4\Downloads\openfusion-reference\2705.1.30`:

- `extrude-invalid-draft.jpg`: invalid draft and linked palette state.
- `extrude-invalid-accessibility.json`: native UI accessibility snapshot, including distinct HUD and panel distance edits.
- `extrude-after-escape.jpg`: state after command cancellation.

The accessibility tree identified the fields through `QTLineEdit_AlongExtrudeDistance_Distance`, with separate HUD and generated-dialog paths. That observation does not establish which shipped QML source, if any, produced those controls.

## Independent implementation contract and gaps

Retain draft input separately from the last valid evaluated value. An invalid draft must remain available for correction and must not destroy the last valid document or geometry. Synchronization of duplicated HUD/palette controls, error styling, and Escape routing require explicit state handling.

Fission 180's current numeric-only editor and lack of a matching HUD remain parity gaps. Expression grammar, units, completion, click-OK behavior with invalid input, delayed validation, preview geometry and transaction internals were not established by this observation. The native modeling investigation addresses a separate static code boundary.

## Unit-bearing expression and measured result

Local observation `RUNTIME-003`, 2026-10-19, resumed the same unsaved scratch design. Extrude had one selected profile and `0.00 mm` in its HUD. The HUD was explicitly focused and its text selected before entry.

1. Replace the selected value with `2 * 3 mm`, then Tab. The palette retained the expression text and OK was enabled.
2. Click OK. The command closed and the Browser acquired a Bodies group containing Body1.
3. Right-click Body1 and open Properties. Fusion reported one body, volume **4800.00 mm³** and surface area **2320.00 mm²**. Those results agree analytically with a 40 × 20 × 6 mm rectangular prism. The displayed precision is two decimal places.
4. Close Properties with Escape. Body1 remains in the unsaved scratch design for review. No cloud save or user-file overwrite was performed.

Retained external images: `extrude-unit-expression.jpg` and `extrude-expression-properties.jpg`. The earlier view remained Top, and this pass does not establish full viewport rendering correctness or an internal parser implementation.

The independent fixture `examples/fusion-reference-extrude.ofrust` stores the evaluated 6 mm distance. `crates/openfusion-core/tests/fusion_reference.rs` compares native OpenCascade body count, volume and surface area to the measured result, checks analytic extents and STEP volume preservation. The core body summary exposes kernel-measured surface area in mm². The test does not establish expression-entry parity or reproduce Fusion's Properties interface.

A subsequent attempt reopened Sketch1 and activated D, but edge selection did not produce a confirmed dimension editor. The command was exited and Finish Sketch restored Solid mode with Body1 present. No driving-dimension edit or dimension delete/undo result is claimed from that attempt. The geometry-edit comparison protocol remains pending.
