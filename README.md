# Green Circle TD

A Rust tower defense game for browsers and desktop, rendered in 3D with wgpu.
The standalone Campaign has 600 authored encounters; Legacy preserves the
36-wave foundation extracted from GREEN TD 9.3c PEIN.w3x.

- WebGPU rendering with WebGL2 fallback, woodland terrain, staged tower models,
  shadows, normal-mapped ground and a responsive RTS command panel.
- Zoom, pan and free placement on valid grass, with tower commands at bottom-right.
- Eleven purchasable tower families, 131 tower definitions and branching upgrades.
- Campaign formations, pressure, commanders, permanent run perks and save/resume.
- Role-based counters reward mixed defenses and focused spending against bosses.

## Run

```sh
cargo install trunk
rustup target add wasm32-unknown-unknown
trunk serve --release --port 8080
```

Open <http://127.0.0.1:8080>. `trunk build --release` produces the static site in
`dist/`. For native play, use `cargo run --release`. Container and lobby setup
is documented in [docs/DEPLOY.md](docs/DEPLOY.md).

## Controls

| Input | Action |
| --- | --- |
| `1`-`9`, `0`, `-` | Select a tower command |
| Click valid grass | Place the armed tower; keep placing until canceled |
| Click a tower | Select it and display its commands |
| Right click / `Esc` | Cancel placement or dismiss supported panels |
| `U` / `S` | Upgrade / sell the selected tower |
| `Enter` | Deploy when allowed |
| `Space` | Pause; commander rewards remain paused until chosen |
| `F` | Cycle speed; Campaign offers 10x, 25x, 50x and 100x |
| `B` / `H` | Cycle quality / show help |
| WASD / arrows / middle drag | Pan the battlefield |
| Mouse wheel | Zoom |

Compact layouts expose commands through the overflow menu. The View control
restores the battlefield framing.

## Campaign and Legacy

Campaign enemies circulate while unresolved bodies contribute to pressure.
Read the next formation and threat details, then choose damage roles, coverage,
control and targeting. Multi excels against crowds, but its additional Campaign
shots deal 40% damage. Armour, shields and air threats call for complementary
towers; tougher Veteran and Nightmare commanders reward focused damage.

Commander rewards pause combat for a damage, attack-speed or range perk.
The campaign ends after all 600 encounters and the final objective are cleared.
Its authored schedule totals 36,360 simulation seconds: about 5 hours 3 minutes
at 2x. The current rapid speed ladder shortens elapsed play time; this timing
regression is not a five-hour human playtest.

Legacy retains the extracted 36 waves and its own economy, speed and perk rules.
Campaign tuning does not silently replace Legacy rules or saved runs.

## Build and verify

```sh
cargo test --locked --release --workspace
trunk build --release
node tools/browser_smoke.mjs http://127.0.0.1:8080
```

The browser smoke harness uses a fresh local Edge profile. See
[the validation record](docs/realistic-campaign/FINAL_VALIDATION.md) for the
verified build, viewport/backend coverage and evidence limits. Optional capture
and diagnostic tests are marked ignored and run explicitly with `--ignored`.

## Source and assets

- `src/game/`: simulation, Campaign authoring, combat, tower definitions and tests.
- `src/gfx/`, `src/view/`, `src/decor.rs`: renderer, materials and 3D scene assembly.
- `src/ui.rs`, `src/main.rs`, `src/save.rs`: controls, application and persistence.
- `assets/`: committed runtime packs and source ground textures.
- `tools/`: map extraction, model authoring/baking and browser smoke checks.
- `proto/`, `server/`: scoreboard/lobby protocol and service.

[Asset credits](assets/CREDITS.md) identify original project meshes and external
sources. Original mesh inputs in `tools/original_meshes/` and both Mosswatch
source textures are retained so the runtime packs can be rebuilt. Map extraction
instructions are in [tools/README.md](tools/README.md); the Warcraft III map itself
is not redistributed and is not required to build or play.

[Implementation status](docs/realistic-campaign/IMPLEMENTATION_STATUS.md) describes
the current release. Superseded task briefs and rejected art captures have been removed.
Local candidate builds, logs, caches and review captures are excluded from Git
and deployment uploads.
