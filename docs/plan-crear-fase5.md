# Plan: Create panel, Phase 5 (dense scenes)

Objective: two new scenes that are born with the richness of the reference illustrations (isometric city, festival, domes under the sea and in space) and do not earn it on the basis of touches like the White village.

**Rule of origin.** They are illustrations with copyright (and real trademarks). They serve only for analysis: no compositions, characters, labels, logos or brands (nothing from Royal Enfield, Toyota, etc.) are copied. The scenes are original, with their own themes and labels. Same criteria as with SVGeometric.

## 1. What brings that wealth (analysis)

1. **Three scales at a time** A huge milestone
   Rocket, circuit), medium blocks (buildings, domes, tribunes) and a powder of
   details (people, cars, motorcycles, balloons, flags, trees). Take off a scale and
   the image is emptying.
2. **Hundreds of people, and readable.** Density doesn't come from drawing too much by
   person, but many very simple people, in groups with intent: a tail,
   A zebra step, a ladder, a lane, a dance. Every group counts.
3. **Every surface does something.** Rooftops with terraces, balconies with people,
   Façades with window, shroud, label and living windows. There 's no smooth roof.
   or empty ground: the void is a decision (a square), not a forgetting.
4. **You're scenes inside the scene.** The glass domes show an interior
   A whole village, a courtyard, a garden. It's the trick that multiplies the
   density without filling the sheet: a simple volume with a world inside.
5. **Thematic districts.** Each area of the plan has its own grammar (port, park,
   Commercial street, circuit). The variety between areas gives a reading; the repetition
   within an orderly zone.
6. **Great way to cross layers.** A giant animal or a pathway that runs through it
   domes, water and sky unite the composition and mark the path of the eye.
7. **The bottom is full, too.** Sea, sky, stars, field: never paper on
   White on the edges. Everything comes to full bleed.
8. **Color as a hierarchy** Few color families per area and a vivid accent. In
   The plotter that's inks: you don't need gamma, you need distribution.

## 2. What to add to the engine and kit (common infrastructure)

It's done first, because both scenes depend on it.

- **Multitude (`kit::crowd`).** Minimum figure of 5  –  8 strokes plus head, with 4  –  6
  Position and height variants, drawn as occlusion sprite (not as solid of
  the revolution). Group generators: `queue`, `crossing`, `stands` (grid by
  grades), `cluster`, `dance`. Budget: 1,000  –  3,000 people per leaf.
- **Vehicles (`kit::vehicle`).** Cars, motorcycles, buses, boats, trains: boxes with profile
  Extruded and wheeled. Placement on a polyline (street, circuit, track) with
  separation and meaning.
- **Façade grammatical (`kit::shopfront`, `kit::tower`).** Window grating
  With rhythms, bass with window and shell, balconies with people, labels and towers
  glass with visible plants. It replaces the current `facade()` in the scenes
  It's very dense.
- **Curved tape (`kit::ribbon`).** Roads, circuits and rails from one
  Soft polyline: band with edge, curb, markings and pearls. It's what we don't today.
  You can make the village (it only works on shafts).
- **Council (`kit::dome`).** Wire cap without insulation (drawn)
  with `whole_line`), with a content which is received as closure: the dome
  It cuts through the inner world and draws its cover above it.
- **Creatures and large shapes (`kit::creature`).** Revolutionary body on a
  Curved axis, with fins or extremities; three species at the beginning (cetaceous,
  the cephalopod, the saurio). Also rocket and station.
- **Labelling.** The labels use the vector text that already exists in the core,
  projected on a façade plan.
- **Budget and measure** `Scene` to 8,000 expensive and 40,000 segments (the bank
  The current measurement is 12 ms with 5,000 and 20,000). Add a density metric by
  Scene: people, meters plotted and meters per square decimeter.
- **Inks.** The games move to 5  –  7 pens: contour, hatching, vegetation,
  land, water, people and labels. Each part retains its ink; with less
  pens are grouped together like they are now.

## 3. Scene A ·  « City at the party »

Isometric, full bleed, with neighborhoods and a circuit that sews it.

- **Plane.** Street grid with a diagonal avenue; four or five
  drawn districts: commercial, residential, park, port or beach; and
  showroom with stage and staircase.
- **Landmarks.** A glass tower with visible floors, a stage with steps
  full and a noria, a circular tank or a lighthouse. Two or three a leaf, never.
  More than that.
- **Circuit.** A curved tape that goes up and down over the blocks, with cars,
  tyres on the curves and slopes.
- **Micro.** Zebra steps with crowds, tails, canopies, roof terraces,
  balloons, flags and trees of three species.
- **Parameters** Dominant district, population density, number of landmarks, presence
  Circuit, beach, park; sunshine and view.
- **How to read with a single ink.** White on the facades of the street (the lime
  4 phase), open hatching on the sides, people and cars in fine stroke.

## 4. Scene B ·  « Domes »

Vertical slide with a waterline (or spatial horizon) and bubbles.

- **Worlds.** Underwater (water, seabed, cetacean, saurian, upper ship) and
  It is also important to note that this is not the case in the European Union. Same engine with another
  decorated and another ink set.
- **domes.** Between five and nine, of different sizes, connected by tunnels. Each
  One contains a micro scene drawn: garden, village, courtyard, market, farm,
  laboratory.
- **Creature that crosses.** A large form (a squid, a whale, a rocket) passes between
  the dome and bind them together in a composite way.
- **Full bottom** Bubbles, fish or stars by density fields, algae or
  Dust clouds, always full bleed.
- **The parameters.** World, number of domes, creature, background density, view.

## 5. Steps

0. **Infrastructure** (the heavy part): `crowd`, `vehicle`, `ribbon`, `dome`,
   `creature`, new façade, budget and density metric, tests of each
   It is not possible to use the product in the same way as the product in question.
   *Made.* `kit::crowd` (6 poses, tails, steps, stairs, rails, dancing and
   Explained; top of 3,000 figures, simplified below 2.6 mm),
   `kit::ribbon` (rope with edges, discontinuous, pianos, track and elevated board),
   `kit::vehicle` (car, motorcycle, bus, boat and attached train), façade
   (`window_grid` with 4 rhythms, balconies with people, windows with blinds,
   signs with the letter of the core, glass tower, roof with umbrellas),
   `kit::dome` (non-covering glass and interior cut to the sphere with
   `Scene::clip_to_sphere`), `kit::creature` (cetaceous, cephalopod, saurian,
   rocket and station), `Scene::load` with the budget of 8,000 expensive and
   40,000 segments, `kit::density` (people, meters and meters per dm2) and the
   ink set `inks::DENSE` from 5 to 7 pens (line, people,
   This is the main reason why it is important for the Commission to take the necessary measures in order to ensure that the measures are implemented effectively. Tests on
   `kit/dense_tests.rs`: deterministic, without NaN, nothing with impossible inputs,
   without seams or overlapping faces, inside the dome inside the glass,
   Preview that clarifies without moving anyone, 3,000 people inside the
   39,652 segments, 15 ms) and sheet at the top in 20 ms.
   `cargo run -p vectorio-core --example kit_denso --release`.
1. Complete **Festival city**, with the 12 seeds rated one by one.
   *Made (shortened version).* Registered `festival_city` generator, with game
   `inks::DENSE`, people controls, 1  –  3 milestones, circuit, view and turn of the sun.
   It includes a core of nine City blocks (one reserved for the square), periphery, shops and rooftops,
   pedestrian crossings, queues, cars, motorcycles and buses; glass tower,
   two-stage stage and circular deposit, drawn unrepeated; and
   urban race through the streets, with fences, public, gradient and goal. The city follows full bleed.
   Determinism, finite geometry within the paper, absence of
   0 seeds  –  11, with all three milestone counts and
   Both circuitry states; also the four views and the ends of people.
   *Improvement:* City blocks continuous facade (one or two rows with courtyard), high landmarks
   behind the square and stage in front, square with fountain, springs, olives and verbena
   (flares and flags crossing the streets), windows with various labels,
   the city, and finished by budget (complete,
   In order to avoid ever going over the top.
   Contactsheet in `target/phase5/contactos.svg` and city details in
   `target/phase5/ciudad.svg`.
   There's the noria, the lighthouse, the beach and the port, the park with three
   In addition, it is important to note that the Commission's proposal for a directive on the
   a detailed evaluation of each seed. This delivery covers the visual review
   Of obvious errors, not that complete artistic evaluation.
   Correction 1: city full bleed and city run; camera to 2.4 - 4 mm/m,
   verified soil coverage and budget-adjusted periphery detail.
   Twelve individual sheets reviewed: buildings to the edge and windows
   They're visible. With this close frame the peripheral run is seen by fragments
   on the edges; the full rectangle is not presented at a single glance.
   Port or promenade, diagonal avenue, noria, parks and balloons remain
   for the next step.
   Correction 2: shadows projected as in the village
   shadows ») and control  « Event »  for the square: verbena; market with twelve
   woven fabrics of cotton, containing predominantly, but < 85% cotton by weight, mixed principally or solely with man-made fibres
   horseback riding on the street of the square closest to the chamber, with carriage,
   Gigants, heads, bands and the public (this section is left without traffic). New
   control  « Living on the street »: work with a half-structure and tower crane (only)
   where the crane fits into the leaf and without covering the front square), balloon
   air conditioners, flocks of pigeons, balloon sellers, neighbourhood shopkeepers
   On the rooftops, painters on hanging scaffolds and benches with people sitting at the
   the foot of the olive trees.
   Cross with the people: the people already use the figures of the city (with posture,
   children and shadow thanks to `Scene::cast_shadow`), and a balloon seller and a
   flocks of pigeons in the square; pigeons, salesman and figure with shadow live
   Now on `kit::life`, shared by the two scenes.
   Loose sheet with manual parameters: `load run -p vector-core --example
   show_recipe --release -- festival_city city.svg 3 event=fair stories= 1 `.
   Correction 3: control  « Costa »  (no coast, beach or port) on one side
   In front of you. Beach with palm walk, umbrellas, bathers, lifeguard,
   Voloes and boats; harbour with pier, longhaul, spigot with lighthouse and
   pants with boats. Control  « Avenue diagonal »  (not with the race): cross the road
   grid in front of the square (behind if there is a coastline), with a median of trees
   and lanterns, and four lanes, and buildings in wedges with planed forearms,
   They are located between the avenue and the chamber) and a park with fountain, cypresses in
   the edge, pine trees at the bottom and olives in front. The camera runs towards the sea and
   to the avenue so they can get on the leaf. Cross with the village: pine trees
   branches between the crops and a pair of palm trees at the entrance to the square.
   With manual parameters the switches go like `si` / `no`:
   `... festival_city avenida.svg 4 avenue=si circuit=no coast=port`.
   4 correction: continuous median between arrivals of streets, with steps
   zebras and pedestrians at each arrival; three-metre-high propellers; and
   Get out of here. With coast and avenue, joint displacement and a 10 % plus
   field to show the water and the mean at the same time. The shopkeepers respect
   now the thickness of the railing at that scale and they don't double on the rooftops.
    « District »  Commercial, residential or leisure lottery: the proportions change
   heights, balconies, shops, courtyards, tents and peripheral terraces;
   The core retains its organization and Leisure favors the stage. Flags and flags
   and on lamps, and on banners on roofs, and on lamps, and on clusters bound at posts and squares.
   The new ornaments are strokes; they share the seller's balloon drawing
   without adding occlusers. Tests of visible vegetation along the axis at the four
   views, and sweep of the three districts with coast and avenue.

   Creating: Liquid Glass floating panel of 360 points, with no removable divider;
   Compact selector and drop-down gallery, legendary ink without side cuts.
   Zoom with pin and buttons, scroll with trackpad and drag; double click
   or  « Adjust »  retrieve the sheet. The preview camera doesn't alter the recipe.
   Off-screen footage of egui on `target/phase5/crear-{980,1440}.png`,
   without closing or restarting the open session. The material uses implementation
   of `theme.rs` glass, including its translucent background and reflections.

   Visual review of 24 sheets (`target/phase5/revision4`), with `--random` and
   I've got my eyes fixed. They don't appreciate people floating on buildings or texts.
   overlapped in this review. The cuts to buildings on the edges are
   intentionally; they have not retreated to simulate a model.

   |View / seed|The Commission shall adopt implementing acts.|
   |---|---|
   |225 / 0|Readable beach and market; visible square, without empty asphalt.|
   |225 / 1|Horseshoe and avenue visible; bow still discreet in front.|
   |225 / 2|Stage and square are well separated; coast without floating elements.|
   |225 / 3|Noria and carousel hide part of the pavement, typical of the event.|
   |225 / 4|Whole harbor and lighthouse on paper; long and legible stage.|
   |225 / 5|Noria partially cut by the frame, without new structural slice.|
   |225 / 6|Works and cranes on one side of the square; building work is open.|
   |225 / 7|Noria and Park compete for attention; it's a densely tuned slope.|
   |225 / 8|Median cups cover part of a passage; trunks outside the crossing.|
   |225 / 9|Open harbor and clear scenery; no city gaps on the edge.|
   |225 / 10|Cranes cut from above, full bleed; spot recognizable next to it.|
   |225 / 11|Dense fair, with noria and stage; separate labels.|
   |315 / 0|Works and beach visible; displaced square, uncovered.|
   |315 / 1|Clear avenue and horse riding; improved but sober bow in this view.|
   |315 / 2|Clear stage and beach; open square.|
   |315 / 3|Dense fair and cropped deposit above; square covered with attractions.|
   |45 / 0|Full beach and stage; cut-out buildings on the edge.|
   |45 / 1|Continuous medium and visible park; large cup reduces passage reading.|
   |45 / 2|Works near the square, which is free; cranes on the right edge.|
   |45 / 3|Noria covers part of the fair; there are no bodies floating over houses.|
   |135 / 0|Work and beach in order; the edge cuts houses, not deserted streets.|
   |135 / 1|Mediana and clear park; cross with visible public.|
   |135 / 2|Square and stage open; work occupies the other side.|
   |135 / 3|Noria dominates the event; readable labels and public around.|

   End of town clearing: middle trees keep 5.5 m with the crosses
   to make room for the cup; the park cups are also checked
   projected on paper so as not to cover the steps. With a fair, the space park
   Its tree-lined and low
   pine and olive trees so they don't compete with the noria. Small parkabicis of two
   bicycles on some sidewalks in the core, linked to  « Life on the street », without
   No additional occlusion or random consumption. Revised all four views
   Fair with avenue and a bicycle view.

2. **domes  —  implemented**, with both worlds and visual references
   the 14 of November. Hierarchy of five to nine domes, connected
   by a tree of gateways, and with six interiors: town with tower, garden,
   courtyard, market, garden and laboratory. Glass in water and contour ink
   dark; the inside is cut into the sphere and the back half hides the
   decorated. A cetacean, squid or marine reptile crosses the underwater bottom; a
   rocket, station or star whale, space. Both can be deactivated.
   Surface water, boat, fish banks, bubbles, coral, rocks, divers and
   small submarine; space with planets, galaxies, stars, lunar horizon,
   craters, small station and astronauts. People controls, fund, number
   of domes and four views. Games Abyss, Orbit and Pencil, with reduction
   of inks according to the number of pens. References are tailored to strokes
   for plotters, without copying their trademarks or characters.
3. **Testing and delivery  —  completed:** 500 seeds per scene without NaN
   or off-sheet coordinates,
   times below 250 ms in Preview and from 1.5 s in Final, clippy, examples,
   installation, contact sheets and `docs/crear-fase5-entrega.md`. Drying
   Additional preview and final domes (maximum 13.50 and 13.15 ms); 24 sheets
   Reviewed from both worlds and egui panel verified in two sizes. Details
   of the tests, clippy advance notices and files on delivery.

It's done in this order: first the people and the tape (the most density brings), and the city with them, before opening the second scene.

## 6. Risks and what I don't promise

- **Plotter time.** A sheet with 2,000 rich people and facades can pass
  of 300 m of stroke. You have to do a density check, and let Preview draw the
  Half of it.
- **Thin lines.** A person to 3 mm is only read with a 0.2  –  0.3 mm pen.
  The minimum size per pen is fixed, as `Style::quantum` already does.
- **Domesters with interior** need the interior to be a cutting world;
  It's the most delicate part of the engine and it can cost more than anything else combined.
- **Texture vs. cleaning.** More density is not better: if the crowd doesn't have
  intentional grouping, it reads like noise. The seed to seed valuation
  It's part of the delivery again.
