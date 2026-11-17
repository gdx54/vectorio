//! Pilas de los siete modos históricos y de los cuatro estilos tonales.
use super::*;
impl Settings {
    pub fn passes(&self) -> Vec<Pass> {
        let visibility = if self.visible_only {
            Visibility::Visible
        } else {
            Visibility::All
        };
        let pass = |name: String, pen, enabled, kind| Pass {
            name,
            pen,
            enabled,
            visibility,
            mask: TonalMask::All,
            dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
            kind,
        };
        let sections = |family| SectionParams {
            field: self.field,
            direction: self.direction,
            cuts: self.cuts,
            spacing: self.spacing,
            offset: self.offset,
            range: self.range,
            major_every: self.major_every,
            family,
            shift_mm: self.strata_shift_mm,
        };
        let waves = |family| WaveParams {
            facets: self.art.facets,
            lines: self.art.lines,
            pattern: self.art.pattern,
            origin_azimuth: self.art.origin_azimuth,
            origin_elevation: self.art.origin_elevation,
            deformation: self.art.deformation,
            frequency: self.art.frequency,
            major_every: self.major_every,
            family,
            outline: self.art.outline,
        };
        let mesh = |group, inks| {
            PassKind::Mesh(MeshParams {
                facets: self.art.facets,
                mesh_facets: self.art.mesh_facets,
                coloring: self.art.coloring,
                inks,
                group,
            })
        };
        match self.recipe {
            RecipeKind::Atlas | RecipeKind::Strata | RecipeKind::XRay => {
                let field = |family| {
                    if self.recipe == RecipeKind::Strata {
                        PassKind::Sections(sections(family))
                    } else {
                        PassKind::Field(sections(family))
                    }
                };
                vec![
                    pass(
                        "Curvas principales".into(),
                        0,
                        self.recipe != RecipeKind::XRay,
                        field(CurveFamily::Primary),
                    ),
                    pass(
                        "Curvas secundarias".into(),
                        1,
                        self.recipe != RecipeKind::XRay,
                        field(CurveFamily::Secondary),
                    ),
                    pass(
                        "Estructura".into(),
                        2,
                        self.recipe == RecipeKind::XRay,
                        PassKind::Edges {
                            angle: self.edge_angle,
                        },
                    ),
                ]
            }
            RecipeKind::FlatShadow => vec![
                pass(
                    "Sombra única · objeto y suelo".into(),
                    0,
                    true,
                    PassKind::Hatch {
                        settings: self.shadow.clone(),
                        middle: false,
                    },
                ),
                pass(
                    "Tono intermedio".into(),
                    1,
                    true,
                    PassKind::Hatch {
                        settings: self.shadow.clone(),
                        middle: true,
                    },
                ),
                pass(
                    "Contorno opcional".into(),
                    2,
                    self.shadow.outline,
                    PassKind::Outline(OutlineParams::Shadow(self.shadow.clone())),
                ),
            ],
            RecipeKind::SurfaceWaves => vec![
                pass(
                    "Ondas principales".into(),
                    0,
                    true,
                    PassKind::Waves(waves(CurveFamily::Primary)),
                ),
                pass(
                    "Ondas secundarias".into(),
                    1,
                    true,
                    PassKind::Waves(waves(CurveFamily::Secondary)),
                ),
                pass(
                    "Silueta".into(),
                    2,
                    self.art.outline,
                    PassKind::Outline(OutlineParams::Surface {
                        facets: self.art.facets,
                    }),
                ),
            ],
            RecipeKind::TriMesh => (0..self.art.mesh_inks.max(3))
                .map(|group| {
                    pass(
                        format!("Malla · grupo {}", group + 1),
                        group,
                        true,
                        mesh(group, self.art.mesh_inks),
                    )
                })
                .collect(),
            RecipeKind::Hybrid => {
                // El estilo conserva su reparto histórico de dos grupos. Las pasadas
                // Mesh y los estilos nuevos admiten tantas tintas como el documento.
                let inks = self.art.mesh_inks.min(2);
                let mut stack = vec![pass(
                    "Ondas de superficie".into(),
                    0,
                    true,
                    PassKind::Waves(waves(CurveFamily::All)),
                )];
                stack.extend((0..2).map(|group| {
                    pass(
                        format!("Malla · grupo {}", group + 1),
                        group + 1,
                        true,
                        mesh(group, inks),
                    )
                }));
                stack
            }
        }
    }
}

/// Estilos tonales de la Fase D. Los siete estilos históricos conservan su pila.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TonalStyle {
    Engraving,
    Crosshatch,
    Technical,
    ShadowTopography,
}
impl TonalStyle {
    pub const ALL: [Self; 4] = [
        Self::Engraving,
        Self::Crosshatch,
        Self::Technical,
        Self::ShadowTopography,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Engraving => "Grabado",
            Self::Crosshatch => "Aguafuerte cruzado",
            Self::Technical => "Plano técnico",
            Self::ShadowTopography => "Topografía en sombra",
        }
    }
    pub fn passes(self, scene: &Settings) -> Vec<Pass> {
        let pass = |name: &str, pen, kind| Pass {
            name: name.into(),
            pen,
            enabled: true,
            visibility: Visibility::Visible,
            mask: TonalMask::All,
            dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
            kind,
        };
        let outline = || {
            pass(
                "Contorno",
                0,
                PassKind::Outline(OutlineParams::Surface {
                    facets: scene.art.facets,
                }),
            )
        };
        match self {
            Self::Engraving | Self::Crosshatch => vec![
                pass(
                    if self == Self::Engraving {
                        "Trama de forma"
                    } else {
                        "Trama cruzada"
                    },
                    0,
                    PassKind::ToneHatch(ToneHatchParams {
                        mode: if self == Self::Engraving {
                            HatchMode::Form
                        } else {
                            HatchMode::Parallel
                        },
                        levels: if self == Self::Engraving { 3 } else { 4 },
                        cuts: 120,
                        ..Default::default()
                    }),
                ),
                outline(),
            ],
            Self::Technical => {
                let visible = pass(
                    "Aristas visibles",
                    0,
                    PassKind::Edges {
                        angle: scene.edge_angle,
                    },
                );
                let mut hidden = pass(
                    "Aristas ocultas",
                    1,
                    PassKind::Edges {
                        angle: scene.edge_angle,
                    },
                );
                hidden.visibility = Visibility::Hidden;
                vec![visible, hidden]
            }
            Self::ShadowTopography => {
                let settings = Settings {
                    cuts: 96,
                    major_every: 1,
                    ..scene.clone()
                };
                let mut curves = settings.passes_for_field();
                curves.name = "Curvas en sombra".into();
                curves.mask = TonalMask::Shadow;
                vec![curves, outline()]
            }
        }
    }
}
impl Settings {
    fn passes_for_field(&self) -> Pass {
        let settings = Self {
            recipe: RecipeKind::Atlas,
            ..self.clone()
        };
        settings.passes().remove(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingStyle {
    VoxelGrid,
    VoxelHatch,
    Curvature,
    Gaussian,
    Light,
    Slope,
    Ambient,
    Thermal,
    Relief,
    Needles,
    Horizons,
    Ridges,
    Anaglyph,
    Unfold,
    Spin,
    Section,
    ScalarContours,
    LightContours,
    OpticalThermal,
    Suggestive,
    ApparentRidges,
    PrincipalFlow,
}
impl ReadingStyle {
    pub const ALL: [Self; 22] = [
        Self::VoxelGrid,
        Self::VoxelHatch,
        Self::Curvature,
        Self::Gaussian,
        Self::Light,
        Self::Slope,
        Self::Ambient,
        Self::Thermal,
        Self::Relief,
        Self::Needles,
        Self::Horizons,
        Self::Ridges,
        Self::Anaglyph,
        Self::Unfold,
        Self::Spin,
        Self::Section,
        Self::ScalarContours,
        Self::LightContours,
        Self::OpticalThermal,
        Self::Suggestive,
        Self::ApparentRidges,
        Self::PrincipalFlow,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::VoxelGrid => "Vóxeles · retícula",
            Self::VoxelHatch => "Vóxeles · caras tramadas",
            Self::Curvature => "Mapa de curvatura",
            Self::Gaussian => "Mapa gaussiano",
            Self::Light => "Mapa de luz",
            Self::Slope => "Mapa de pendientes",
            Self::Ambient => "Luz ambiente",
            Self::Thermal => "Mapa térmico",
            Self::Relief => "Hachuras de relieve",
            Self::Needles => "Mapa de agujas",
            Self::Horizons => "Líneas de horizonte",
            Self::Ridges => "Crestas y valles",
            Self::Anaglyph => "Anaglifo",
            Self::Unfold => "Desarrollo",
            Self::Spin => "Estela de giro",
            Self::Section => "Sección técnica",
            Self::ScalarContours => "Isolíneas de curvatura",
            Self::LightContours => "Isolíneas de luz",
            Self::OpticalThermal => "Mezcla óptica térmica",
            Self::Suggestive => "Contornos sugeridos",
            Self::ApparentRidges => "Aristas aparentes",
            Self::PrincipalFlow => "Trazos de curvatura",
        }
    }
    pub fn passes(self, _scene: &Settings) -> Vec<Pass> {
        let mut map = ReadingMap::default();
        let mut params = ReadingParams::default();
        match self {
            Self::VoxelGrid | Self::VoxelHatch => {
                let hatch = self == Self::VoxelHatch;
                map.measure = Measure::Objects;
                map.ramp = Ramp::Categorical;
                map.pens = if hatch { vec![6, 2, 4] } else { vec![4, 2, 6] };
                params.effect = ReadingEffect::Voxels { divisions: if hatch { 13 } else { 27 }, hatch, spacing_mm: 0.4 };
            }
            Self::PrincipalFlow => {
                map.ramp = Ramp::Sequential;
                map.pens = vec![0];
                params.facets = 8000;
                params.effect = ReadingEffect::PrincipalFlow {
                    minimum: false,
                    spacing_mm: 2.4,
                    length_mm: 22.,
                    smoothing: 1,
                };
            }
            Self::Suggestive | Self::ApparentRidges => {
                map.ramp = Ramp::Sequential;
                map.pens = vec![0];
                params.facets = 8000;
                params.effect = if self == Self::Suggestive {
                    ReadingEffect::Suggestive {
                        threshold: 0.1,
                        min_angle_deg: 20.,
                        smoothing: 1,
                    }
                } else {
                    ReadingEffect::ApparentRidges {
                        threshold: 8.,
                        smoothing: 3,
                    }
                };
            }
            Self::ScalarContours => {
                map.encoding = Encoding::Isolines;
                map.levels = 20;
            }
            Self::LightContours => {
                map.encoding = Encoding::Isolines;
                map.measure = Measure::Light;
                map.ramp = Ramp::Sequential;
                map.pens = vec![0, 2, 4];
                map.levels = 20;
            }
            Self::OpticalThermal => {
                map.encoding = Encoding::Optical;
                map.measure = Measure::Height;
                map.ramp = Ramp::Sequential;
                map.pens = vec![2, 1, 4, 0];
                params.facets = 8000;
            }
            // Con la rampa divergente, cada signo se reparte por separado: una forma
            // convexa sigue mostrando todo su relieve en lugar de un solo color.
            Self::Curvature => {
                map.equalize = true;
                map.pens = vec![2, 3, 1, 4];
            }
            Self::Gaussian => {
                map.measure = Measure::GaussianCurvature;
                map.equalize = true;
                map.pens = vec![2, 3, 1, 4];
            }
            Self::Light => {
                map.measure = Measure::Light;
                map.ramp = Ramp::Sequential;
                map.pens = vec![0, 2, 3, 4];
            }
            Self::Slope => {
                map.measure = Measure::Slope;
                map.ramp = Ramp::Sequential;
                map.pens = vec![0, 2, 3, 4];
            }
            Self::Ambient => {
                map.measure = Measure::AmbientOcclusion;
                map.ramp = Ramp::Sequential;
                map.encoding = Encoding::Density;
                map.pens = vec![0];
                map.spacing_mm = 0.5;
                params.facets = 8000;
            }
            Self::Thermal => {
                map.measure = Measure::Height;
                map.ramp = Ramp::Sequential;
                map.pens = vec![2, 3, 5, 4];
            }
            Self::Relief => {
                map.measure = Measure::Slope;
                map.ramp = Ramp::Sequential;
                map.pens = vec![0, 2];
                params.effect = ReadingEffect::Relief { length_mm: 4. };
            }
            Self::Needles => {
                map.measure = Measure::Slope;
                map.ramp = Ramp::Sequential;
                map.pens = vec![2, 3, 5];
                params.effect = ReadingEffect::Needles { size_mm: 3. };
            }
            Self::Horizons => {
                map.measure = Measure::Depth;
                map.ramp = Ramp::Sequential;
                map.pens = vec![0, 2, 3];
                params.effect = ReadingEffect::Horizons {
                    height_mm: 24.,
                    rows: 96,
                };
            }
            Self::Ridges => {
                params.effect = ReadingEffect::Ridges { threshold: 0.05 };
            }
            Self::Anaglyph => {
                map.ramp = Ramp::Diverging;
                map.pens = vec![1, 2];
                params.effect = ReadingEffect::Anaglyph {
                    angle_deg: 3.,
                    side_by_side: false,
                };
            }
            Self::Unfold => {
                map.pens = vec![0, 1, 2];
                params.facets = 512;
                params.effect = ReadingEffect::Unfold { tabs_mm: 2. };
            }
            Self::Spin => {
                map.ramp = Ramp::Sequential;
                map.pens = vec![2, 1, 5, 4];
                params.effect = ReadingEffect::Spin {
                    angle_deg: 12.,
                    copies: 4,
                };
            }
            Self::Section => {
                map.pens = vec![0, 2, 1];
                params.effect = ReadingEffect::Section {
                    direction: [0., 0., 1.],
                    offset: 0.,
                    show_dimensions: true,
                };
            }
        }
        let facets = params.facets;
        let mut stack = vec![Pass {
            name: self.name().into(),
            enabled: true,
            pen: 0,
            visibility: Visibility::Visible,
            mask: TonalMask::All,
            dash: DashPattern::default(),
            reading: Some(map),
            finish: FinishSettings::default(),
            kind: PassKind::Reading(params),
        }];
        if matches!(
            self,
            Self::Suggestive | Self::ApparentRidges | Self::PrincipalFlow
        ) {
            stack.push(Pass::new(
                "Contorno",
                PassKind::Outline(OutlineParams::Surface { facets }),
                0,
            ));
        }
        stack
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtisticStyle {
    Halo,
    Echo,
    Hand,
    Gradient,
}
impl ArtisticStyle {
    pub const ALL: [Self; 4] = [Self::Halo, Self::Echo, Self::Hand, Self::Gradient];
    pub fn name(self) -> &'static str {
        match self {
            Self::Halo => "Ondas con halo",
            Self::Echo => "Eco de contorno",
            Self::Hand => "Trazo a mano",
            Self::Gradient => "Ondas con gradiente",
        }
    }
    pub fn passes(self, scene: &Settings) -> Vec<Pass> {
        let settings = Settings {
            recipe: RecipeKind::SurfaceWaves,
            major_every: 1,
            ..scene.clone()
        };
        let mut waves = settings.passes().remove(0);
        waves.pen = 0;
        waves.name = self.name().into();
        if let PassKind::Waves(p) = &mut waves.kind {
            p.family = CurveFamily::All;
            p.lines = 64;
            p.outline = false;
        }
        let outline = Pass {
            name: "Contorno".into(),
            enabled: true,
            pen: 0,
            visibility: Visibility::Visible,
            mask: TonalMask::All,
            dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
            kind: PassKind::Outline(OutlineParams::Surface {
                facets: scene.art.facets,
            }),
        };
        match self {
            Self::Halo => {
                waves.finish.halo_mm = 1.2;
                vec![waves, outline]
            }
            Self::Echo => {
                let mut outline = outline;
                outline.name = self.name().into();
                outline.finish.echo_count = 3;
                outline.finish.echo_mm = 0.8;
                vec![outline]
            }
            Self::Hand => {
                waves.finish = FinishSettings {
                    jitter_mm: 0.12,
                    irregularity: 0.15,
                    gap_probability: 0.06,
                    seed: 17,
                    ..Default::default()
                };
                let mut outline = outline;
                outline.finish.jitter_mm = 0.06;
                outline.finish.seed = 17;
                vec![waves, outline]
            }
            Self::Gradient => {
                waves.reading = Some(ReadingMap {
                    measure: Measure::Height,
                    ramp: Ramp::Sequential,
                    encoding: Encoding::Dither,
                    pens: vec![2, 5, 4],
                    ..Default::default()
                });
                vec![waves]
            }
        }
    }
}
