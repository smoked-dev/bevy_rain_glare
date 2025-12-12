#![expect(
    dead_code,
    reason = "ShaderType derive emits internal helpers named `check`."
)]

use bevy::{
    asset::load_internal_asset,
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::query::QueryItem,
    prelude::*,
    render::{
        RenderApp,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice},
        texture::BevyDefault,
        view::ViewTarget,
    },
    utils::HashMap,
};

/// Handle for the internally embedded rain glare shader.
pub const RAIN_GLARE_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0xA6D4_91D1_D6C3_44FD_821D_A4A6_9B0A_9B11);

/// Component that enables the rain glare effect on a camera and configures its parameters.
#[allow(dead_code)]
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub struct RainGlareSettings {
    pub intensity: f32,
    pub threshold: f32,
    pub streak_length_px: f32,
    pub rain_density: f32,

    pub wind: Vec2,
    pub speed: f32,
    pub time: f32,

    // NEW: smaller pattern = bigger scale (3.0 => ~3x smaller features)
    pub pattern_scale: f32,
    // NEW: hard-edged line thickness in pixels (keep ~0.5..1.25)
    pub mask_thickness_px: f32,
    // NEW: snap streak sampling to pixel centers (1.0 = on, 0.0 = off)
    pub snap_to_pixel: f32,
    // NEW: quantize mask intensity steps (0 = off, 8/16 = crunchy)
    pub tail_quant_steps: f32,

    /// 0..1: how “horizon-facing” the view is.
    /// 1 = looking at horizon, 0 = straight up/down.
    pub view_angle_factor: f32,
}

impl Default for RainGlareSettings {
    fn default() -> Self {
        Self {
            intensity: 0.35,
            threshold: 0.65,
            streak_length_px: 96.0,
            rain_density: 0.55,
            wind: Vec2::new(0.10, 1.0),
            speed: 1.2,
            time: 0.0,

            pattern_scale: 3.0,
            mask_thickness_px: 0.75,
            snap_to_pixel: 1.0,
            tail_quant_steps: 8.0,
            
            view_angle_factor: 1.0,
        }
    }
}

/// Plugin that wires the rain glare effect into the render graph.
pub struct RainGlarePlugin;

impl Plugin for RainGlarePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            RAIN_GLARE_SHADER_HANDLE,
            "../assets/rain_glare.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins((
            ExtractComponentPlugin::<RainGlareSettings>::default(),
            UniformComponentPlugin::<RainGlareSettings>::default(),
        ))
        // Keep the time parameter in sync with the engine clock.
        .add_systems(Update, advance_rain_time);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_graph_node::<ViewNodeRunner<RainGlareNode>>(Core3d, RainGlareLabel)
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::Tonemapping,
                    RainGlareLabel,
                    Node3d::EndMainPassPostProcessing,
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<RainGlarePipeline>();
    }
}

#[derive(Default)]
struct RainGlareNode;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct RainGlareLabel;

impl ViewNode for RainGlareNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static RainGlareSettings,
        &'static DynamicUniformIndex<RainGlareSettings>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _settings, settings_index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<RainGlarePipeline>();
        let view_format = view_target.main_texture_format();

        let Some(pipeline_id) = pipeline.pipeline_for_format(view_format) else {
            return Ok(());
        };

        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(render_pipeline) = pipeline_cache.get_render_pipeline(*pipeline_id) else {
            return Ok(());
        };

        let settings_uniforms = world.resource::<ComponentUniforms<RainGlareSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "rain_glare_bind_group",
            &pipeline.layout,
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline.sampler,
                settings_binding.clone(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("rain_glare_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[derive(Resource)]
struct RainGlarePipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipelines: HashMap<TextureFormat, CachedRenderPipelineId>,
}

impl RainGlarePipeline {
    fn pipeline_for_format(&self, format: TextureFormat) -> Option<&CachedRenderPipelineId> {
        self.pipelines.get(&format)
    }
}

impl FromWorld for RainGlarePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "rain_glare_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<RainGlareSettings>(true),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());
        let shader = RAIN_GLARE_SHADER_HANDLE.clone();

        let mut pipelines = HashMap::new();
        let pipeline_cache = world.resource_mut::<PipelineCache>();
        for format in [
            TextureFormat::bevy_default(),
            ViewTarget::TEXTURE_FORMAT_HDR,
        ] {
            let id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("rain_glare_pipeline".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader: shader.clone(),
                    shader_defs: vec![],
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
            });
            pipelines.insert(format, id);
        }

        Self {
            layout,
            sampler,
            pipelines,
        }
    }
}

/* fn advance_rain_time(time: Res<Time>, mut query: Query<&mut RainGlareSettings>) {
    for mut settings in &mut query {
        settings.time += time.delta_seconds();
    }
} */
fn advance_rain_time(
    time: Res<Time>,
    mut q: Query<(&GlobalTransform, &mut RainGlareSettings), With<Camera3d>>,
) {
    let t = time.elapsed_seconds();

    for (global_transform, mut settings) in &mut q {
        settings.time = t;

        // World-space view direction (forward).
        // GlobalTransform::forward() returns Dir3; convert to Vec3.
        let forward: Vec3 = global_transform.forward().into();

        // World up (assuming Y-up). Change if you use a different up-axis.
        let world_up = Vec3::Y;

        // How much the camera is pointing up/down.
        let vertical = forward.dot(world_up);           // -1..1
        let horizon = (1.0 - vertical.abs()).clamp(0.0, 1.0);

        // Sharpen so it’s strong near the horizon, fades faster near zenith/nadir.
        let exponent = 2.0;
        let angle_factor = horizon.powf(exponent);

        settings.view_angle_factor = angle_factor;
    }
}