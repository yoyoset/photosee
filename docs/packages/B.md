# B: slideshow state machine (web, pure)

Read `AGENTS.md` first. Owned paths: see `[B]` in `docs/ownership.toml`.

## Why
The slideshow is the one feature the user asked to configure: the interval,
and what happens at the end of a folder. Keeping the rules in a pure
function lets every edge case be a table test. The timer and DOM live in
`slideshow-ui.ts` (package L), and that file calls `step`.

## Implement `web/src/slideshow.ts` `step(s, ev, cfg, rand)`
Pure: no timers, no DOM, no `Math.random` (use `rand`, which returns
[0,1)). You may extend `SlideEvent` and `SlideEffect` (you own the file),
but do not change `step`'s signature. Rules (`docs/ux-spec.md` §Slideshow):

| state | event | result |
|---|---|---|
| off | start{index,count} | playing; `order` = identity, or when `cfg.shuffle`: `[index, …Fisher–Yates of the rest]`; pos 0; effect none (the current image is already on screen; the UI starts the timer) |
| off | start with count 0 | stays off; `stopped{end}` |
| playing | tick or image_failed | if pos+1 < order.length: pos+1, `show{order[pos+1]}` |
| playing | tick/image_failed at last pos | on_end `loop`: pos 0, `show{order[0]}` (keep the same order) · `next_folder`: stay playing, `load_next_folder` · `stop`: off, `stopped{end}` |
| playing (waiting for folder) | folder_loaded{count>0} | new order for 0..count (shuffled when cfg.shuffle, starting from a random image), pos 0, `show{order[0]}` |
| playing | folder_loaded{count 0} | `load_next_folder` again (the server already skips empty folders, so this is defensive) |
| playing | add `no_next_folder` | off, `stopped{end}` |
| playing | pause | paused |
| paused | resume | playing (effect none; the UI restarts the timer) |
| any non-off | stop | off, `stopped{user}` |
| any | an event that does not apply | unchanged state, effect none |

Remember that `pos` indexes `order`, not the image list.

## Tests (`web/test/slideshow.test.ts`)
- The full table above, row by row.
- Shuffle is a permutation: no index is lost or repeated, it starts with the
  current index, and it is deterministic for a seeded `rand`.
- `tick` while paused and `image_failed` while off are ignored.
- A long run: start → 1000 ticks with on_end loop → still a valid state.

## Acceptance
`powershell -File scripts/check.ps1 -Web` and
`node scripts/gates.mjs --ownership B main` pass.

## Not in scope
Timers, the interval setting UI, the folder toast (package L), and calling
`/nav` (controller, Wave 2).
