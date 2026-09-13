# Implementation status

Verified release: `woodland-command-tempo-20260913.100` (2026-09-13).

- The 600-encounter Campaign runs alongside the separate 36-wave Legacy mode.
- The rectangular woodland battlefield supports zoom, pan and free-grass placement.
  Responsive controls retain the bottom-right tower command grid.
- Campaign formations, role counters, pressure, commander mechanics, run perks and
  save/resume are implemented. Multi extra shots deal 40% Campaign damage, and the
  Poison rider uses 10% of its Legacy DPS with matching UI values.
- Veteran/Nightmare commanders use an encounter-based minimum health, independent
  of the player's wealth or board. Commander trigger state resets between encounters.
- The wave card has two text rows and a separate progress bar. Pressure cannot show
  negative zero. The compact reward panel remains paused until a perk is selected;
  valid later-chapter and concentrated perk ranks restore from saves.
- The authored schedule is 36,360 simulation seconds, about 5 hours 3 minutes at 2x.
  The current HUD offers 10x/25x/50x/100x speeds, so actual elapsed time differs.

See [FINAL_VALIDATION.md](FINAL_VALIDATION.md) for checks and limitations.
Historical design and correction documents record the evolving brief; use this
status and production code when determining what is currently implemented.
