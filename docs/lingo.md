# Lingo — the Dead-taper glossary, applied

`warp-taper`'s vocabulary borrows from Grateful Dead bootleg-tape culture
because the shape of the problem is the same: a recording is made, archived,
shared, and indexed against other recordings.

## Source vocabulary

| Term | Dead-taper meaning | `warp-taper` meaning |
|---|---|---|
| **Tape** | A single recording of one show | An evidence bundle for one scenario, in `tapes/<scenario>/` |
| **Vault** | The Dead's official tape archive | The `tapes/` directory itself |
| **Master** | The original, highest-quality recording (the taper's own copy) | The primary screen capture of a session: `tapes/<scenario>/master.mov` |
| **Patch** | A clean replacement segment, often from a different source | A named still snapshot at a specific point in the scenario: `tapes/<scenario>/patches/<name>.png` |
| **SBD** | "Soundboard" — a direct feed from the venue's mixing board, highest fidelity | A direct log capture of `warp-oss.log` during the session — ground truth, no UI translation |
| **AUD** | "Audience" — a microphone recording from the crowd, captures the room | The screen recording — captures what the user actually saw on screen |
| **Matrix** | A mix of SBD + AUD, blending fidelity and ambience | The bundle as a whole: SBD logs + AUD recording side by side |
| **Setlist** | The ordered list of songs played at a show | The ordered steps in `scenario.md` |
| **Set break** | The pause between sets | The setup/teardown phase between the build and record stages |
| **Encore** | The bonus closing song(s) | Optional follow-up assertions in `assertions.sh` after the main bundle |
| **Generation** | How many tape-to-tape copies away from the master (1st-gen, 2nd-gen, …) | Re-bundling: each `warp-taper bundle` regenerates the README from the existing artifacts |
| **Trade** | The community of taper-to-taper sharing under the Dead's open-tape policy | Attaching a tape to a PR — open trading of evidence |

## Things `warp-taper` deliberately does NOT borrow

- **Wall of Sound** is too cool. Maybe a future PA-scale capture mode. Not now.
- **Steal Your Face** as an icon was tempting but feels off-topic.
- **Cease-and-desist** has no equivalent — `warp-taper` only records your own
  Warp instance against your own checkout. There's nothing to police.

## Why this is more than a joke

The shape of evidence really does match the shape of a concert recording:

1. There's an event in time that you can't replay (a runtime behavior, a UI
   moment, a log emission). You either captured it or you didn't.
2. The capture has fidelity tiers (screen vs log; AUD vs SBD).
3. The archive has value over time — past tapes give context for future tapes.
4. Sharing is the point. The recording exists so other people can see what
   happened.

A reviewer reading "tape" in a PR comment instantly knows it's an evidence
bundle, not a literal cassette. The metaphor pays for itself.
