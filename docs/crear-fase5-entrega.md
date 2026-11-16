# Create · Phase 5 delivery

14 of November of 2026.

## Scenes

**Party City** preserves its events, districts, coastline, diagonal avenue, street life, label control and pen game. Details of your visual review and subsequent adjustments are in `plan-crear-fase5.md`.

**Domes** adds a separate recipe, editable from Create and saved with the document like the others. References provided by the user guide the hierarchy of sizes, inhabited worlds and protagonists; no raster images, brands or characters are incorporated into the references.

- Five to nine semi-spherical domes with sockles, glass nerves and walkways.
- Six interiors: town with tower, garden with fountain, courtyard, market,
  Garden and laboratory. The sphere cuts its geometries and the rear glass
  conceals the decoration, keeping the interior objects visible.
- Underwater world with surface, boat, fish, bubbles, reef, rocks,
  divers and a small submarine. Whale, squid or sea reptile as protagonist.
- Space world with planets, galaxies, stars, lunar horizon, craters,
  station and astronauts. Rocket, orbital station or star whale.
- You can dispense with the protagonist, regulate people and background and choose four
  Views. The locks and seed follow the common behavior of Create.
- Games Abyss, Orbit and Pen; reduction of inks and reuse of ink
  pen without mixing objects with inks. Changes in the world
  the user's choice of inks.

Shared pieces gain traits projected onto the creatures' skin to avoid eyes and mouths cut by their own body, and rocket windows. The glass uses the  « water »  part, which each game solves in its corresponding ink.

## Checks and inspections

- `cargo test --workspace --release`: 214 Tests passed (core 171,
  42 interface and one documentation), eight manual checks ignored
  By default. The two tests of 500 seeds were executed explicitly.
- Drying of 500 seeds by each of the five generators: coordinates
  finites within the area and less than 80 m; no construction notices in Pueblo,
  Neither city nor domes.
- Additional soaking of domes: 500 seeds in Preview and Final. Maximum of
  13.50 ms and 13.15 ms respectively in this equipment, below 250 ms and
  1.5 The following points shall be inserted: These times measure generation, without rendering the interface.
- Specific cases of both worlds, all the protagonists, four views,
  five to nine domes, vertical/horizontal paper and 0.7 mm pen;
  It is also important to note that the Commission's proposals in this field are based on a number of factors.
  segments. Full eye regression over the curved body in four views.
- All examples compiled in release.
- `cargo clippy --workspace --all-targets` ends correctly. Save warnings
  previously in `doc`, `fill`, `optimize`, `raster`, `svg_in`, `vectorize`,
  `muestra_texto`, `canvas` and a test of `vectorize_ui`; no warnings on domes
  Not in the modified parts here. The execution with `-D warnings` remains
  blocked by those existing notices, not this scene.

## Visual and archival review

Individual sheets and twelve seeds checked by each world in `target/phase5/domes`, with `contactos-underwater.svg` and `contactos-space.svg` sheets. No background leaks through the interiors or unexpected cuts in the domes are observed on these 24 sheets. The protagonists cross behind the worlds; some windows are hidden from them. Captures of the egui itself, without opening another instance or touching the user window: `target/phase5/cupulas-{underwater,space}-{980,1440}.png`.

The adaptation is linear drawing for pen: it does not reproduce the large color masses of the references. The protagonists intersect behind the domes and remain hidden where appropriate; the coral and part of the creatures can continue full bleed on the edge of the leaf.

Complete installation and verified signature; source footprint matches the installation. It 's updated with `python3 scripts/install-macos.py` on `/Applications/Vectorio.app`. The open window retains its version until the user reopens the application.
