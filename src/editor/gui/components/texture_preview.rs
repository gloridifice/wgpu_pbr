use egui::{Color32, Vec2, load::SizedTexture};

/// Cached state for previewing a wgpu texture in egui.
///
/// Caches the registered `egui::TextureId` and the current array-layer view
/// so they aren't recreated every frame.
pub struct TexturePreview {
    egui_tex_id: Option<egui::TextureId>,
    cached_view: Option<wgpu::TextureView>,
    cached_layer: u32,
}

impl TexturePreview {
    pub fn new() -> Self {
        Self {
            egui_tex_id: None,
            cached_view: None,
            cached_layer: 0,
        }
    }

    /// Invalidate all cached state. Next show call will re-register the texture.
    pub fn invalidate(&mut self) {
        self.egui_tex_id = None;
        self.cached_view = None;
    }

    /// Display a pre-created texture view with format and size metadata.
    ///
    /// Call this from inside an egui UI closure. The image scales to fill
    /// the available space while preserving aspect ratio.
    pub fn show_view(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &mut egui_wgpu::Renderer,
        device: &wgpu::Device,
        texture_view: &wgpu::TextureView,
        texture_size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
    ) {
        egui::Frame::dark_canvas(ui.style())
            .inner_margin(egui::Vec2::new(8., 6.))
            .show(ui, |ui| {
                self.header_ui(ui, format, texture_size);

                if texture_size.width == 0 || texture_size.height == 0 {
                    ui.colored_label(Color32::YELLOW, "Zero-size texture");
                    return;
                }

                ui.add_space(4.0);

                let available = ui.available_size();
                let display_size = fit_size(
                    [texture_size.width as f32, texture_size.height as f32],
                    available,
                );

                if display_size.x < 1.0 || display_size.y < 1.0 {
                    return;
                }

                if self.egui_tex_id.is_none() {
                    self.egui_tex_id = Some(renderer.register_native_texture(
                        device,
                        texture_view,
                        wgpu::FilterMode::Linear,
                    ));
                }

                if let Some(tex_id) = self.egui_tex_id {
                    ui.image(SizedTexture::new(tex_id, display_size));
                }
            });
    }

    /// Display a texture with array-layer browsing controls.
    ///
    /// When `depth_or_array_layers > 1`, prev / next buttons let you cycle
    /// through layers. The component creates a `D2` view for the selected
    /// layer internally.
    pub fn show_texture(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &mut egui_wgpu::Renderer,
        device: &wgpu::Device,
        texture: &wgpu::Texture,
    ) {
        let size = texture.size();
        let format = texture.format();
        let total_layers = size.depth_or_array_layers;

        // Clamp layer when the source texture changes to a smaller array.
        if self.cached_layer >= total_layers {
            self.cached_layer = 0;
            self.invalidate();
        }

        egui::Frame::dark_canvas(ui.style())
            .inner_margin(egui::Vec2::new(8., 6.))
            .show(ui, |ui| {
                self.header_ui(ui, format, size);

                // Layer selector for array textures
                if total_layers > 1 {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label("Layer:");
                        if ui
                            .add_enabled(
                                self.cached_layer > 0,
                                egui::Button::new("◀"),
                            )
                            .clicked()
                        {
                            self.cached_layer = self.cached_layer.saturating_sub(1);
                            self.invalidate();
                        }
                        ui.label(format!(
                            "{} / {}",
                            self.cached_layer,
                            total_layers.saturating_sub(1)
                        ));
                        if ui
                            .add_enabled(
                                self.cached_layer + 1 < total_layers,
                                egui::Button::new("▶"),
                            )
                            .clicked()
                        {
                            self.cached_layer += 1;
                            self.invalidate();
                        }
                    });
                }

                if size.width == 0 || size.height == 0 {
                    ui.colored_label(Color32::YELLOW, "Zero-size texture");
                    return;
                }

                ui.add_space(4.0);

                let available = ui.available_size();
                let display_size = fit_size(
                    [size.width as f32, size.height as f32],
                    available,
                );

                if display_size.x < 1.0 || display_size.y < 1.0 {
                    return;
                }

                // Rebuild the view when the layer changes.
                if self.cached_view.is_none() {
                    self.cached_view =
                        Some(create_single_layer_view(texture, self.cached_layer));
                }

                if let Some(ref view) = self.cached_view {
                    if self.egui_tex_id.is_none() {
                        self.egui_tex_id = Some(renderer.register_native_texture(
                            device,
                            view,
                            wgpu::FilterMode::Linear,
                        ));
                    }

                    if let Some(tex_id) = self.egui_tex_id {
                        ui.image(SizedTexture::new(tex_id, display_size));
                    }
                }
            });
    }

    fn header_ui(&self, ui: &mut egui::Ui, format: wgpu::TextureFormat, size: wgpu::Extent3d) {
        ui.horizontal(|ui| {
            ui.label("Format:");
            ui.colored_label(format_color(format), format!("{:?}", format));
            if is_depth_format(format) {
                ui.colored_label(Color32::from_rgb(255, 165, 0), "(depth)");
            }
        });
        ui.horizontal(|ui| {
            ui.label("Size:");
            ui.colored_label(Color32::LIGHT_BLUE, format!("{}x{}", size.width, size.height));
            if size.depth_or_array_layers > 1 {
                ui.colored_label(
                    Color32::LIGHT_BLUE,
                    format!("x{}", size.depth_or_array_layers),
                );
            }
        });
    }
}

/// Simplified free function for one-shot texture display without state caching.
///
/// Re-registers the texture with egui every frame. Prefer [`TexturePreview::show_view`]
/// for repeated use.
pub fn texture_preview_image(
    ui: &mut egui::Ui,
    renderer: &mut egui_wgpu::Renderer,
    device: &wgpu::Device,
    texture_view: &wgpu::TextureView,
    texture_size: [u32; 2],
) {
    let tex_id =
        renderer.register_native_texture(device, texture_view, wgpu::FilterMode::Linear);

    let available = ui.available_size();
    let display_size =
        fit_size([texture_size[0] as f32, texture_size[1] as f32], available);

    if display_size.x >= 1.0 && display_size.y >= 1.0 {
        ui.image(SizedTexture::new(tex_id, display_size));
    }
}

/// Returns `true` if the format is a depth or stencil format.
pub fn is_depth_format(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Depth16Unorm
            | wgpu::TextureFormat::Depth24Plus
            | wgpu::TextureFormat::Depth24PlusStencil8
            | wgpu::TextureFormat::Depth32Float
            | wgpu::TextureFormat::Depth32FloatStencil8
            | wgpu::TextureFormat::Stencil8
    )
}

/// Returns `true` if the format can be meaningfully displayed as a color image.
pub fn is_displayable_color_format(format: wgpu::TextureFormat) -> bool {
    !is_depth_format(format)
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn create_single_layer_view(texture: &wgpu::Texture, layer: u32) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: layer,
        array_layer_count: Some(1),
        ..Default::default()
    })
}

fn fit_size(tex_size: [f32; 2], container: Vec2) -> Vec2 {
    let [tw, th] = tex_size;
    if tw <= 0.0 || th <= 0.0 {
        return Vec2::new(container.x.min(256.0), container.y.min(256.0));
    }
    let aspect = tw / th;
    let cw = container.x;
    let ch = container.y;
    if cw / ch > aspect {
        Vec2::new(ch * aspect, ch)
    } else {
        Vec2::new(cw, cw / aspect)
    }
}

fn format_color(format: wgpu::TextureFormat) -> Color32 {
    if is_depth_format(format) {
        Color32::from_rgb(255, 165, 0)
    } else {
        Color32::LIGHT_GREEN
    }
}
