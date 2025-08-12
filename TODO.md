start_original

- fix all examples
- investigate state of repository
- build up more examples
- make sure that we have modular API that can easily have backend changed
- work on "engine" features, maybe as a layer above this
- start work on game (2d cooking card game)

end_original

FUTURE PLAN:

tldr; innovative css-like layout ontop of render space

Plan: add a minimal, composable layout pipeline on top of render space:
Anchors to screen/container edges (top/left/right/bottom, center).
Percent sizing and margins/padding.
Relative-to-object constraints (“align left of X”, “center to Y”).
Later: grid/flex containers if needed.
Innovative but safe: start with anchors/percent/relative (battle-tested patterns), layer on “layout passes”
before queueing draws. Camera only affects render space, not layout evaluation.
