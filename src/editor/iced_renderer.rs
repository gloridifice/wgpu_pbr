use bevy_ecs::prelude::*;
use bevy_ecs::query::With;
use bevy_ecs::world::FromWorld;
use iced::mouse::Cursor as IcedCursor;
use iced_wgpu::{Engine, Renderer};
use iced_winit::core::{clipboard::Null, Font, Pixels, Size as IcedSize};
use iced_winit::core::widget::operation::focusable;
use iced_wgpu::graphics::Viewport;
use iced_runtime::user_interface::{Cache, UserInterface};
use lentille_core::window::{PrimaryWindow, WinitWindow};
use lentille_render::{RenderState, SCREEN_FORMAT};
use winit::window::Window;

use crate::editor::data_types::{EditorUiState, IcedMessage, ThreadLocal};
use crate::editor::palette::{RENAME_INPUT_ID, PROPERTY_INPUT_ID};
use crate::editor::view::build_editor_ui;

/// Holds the iced wgpu renderer and the runtime state needed to update and
/// draw the editor UI on top of the scene.
#[derive(Resource)]
pub(crate) struct IcedRenderer {
    pub renderer: Renderer,
    pub theme: iced::Theme,
    pub viewport: Viewport,
    pub events: Vec<iced::Event>,
    pub cursor: IcedCursor,
    pub modifiers: winit::keyboard::ModifiersState,
    pub ui_cache: ThreadLocal<Cache>,
}

impl FromWorld for IcedRenderer {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let window = world
            .query_filtered::<&WinitWindow, With<PrimaryWindow>>()
            .single(world)
            .unwrap()
            .0
            .clone();

        let rs = world.resource::<RenderState>();
        Self::new(&rs.device, &rs.queue, &rs.adapter, &window)
    }
}

impl IcedRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        adapter: &wgpu::Adapter,
        window: &Window,
    ) -> Self {
        let engine = Engine::new(
            adapter,
            device.clone(),
            queue.clone(),
            SCREEN_FORMAT,
            None,
            iced_wgpu::graphics::Shell::headless(),
        );

        let renderer = Renderer::new(engine, Font::default(), Pixels(16.0));

        // Load MiSans font if available
        if let Ok(font_bytes) = std::fs::read("assets/fonts/MiSans-Normal.ttf") {
            let font_sys = iced_wgpu::graphics::text::font_system();
            font_sys
                .write()
                .unwrap()
                .load_font(std::borrow::Cow::Owned(font_bytes));
        }

        let physical_size = window.inner_size();
        let viewport = Viewport::with_physical_size(
            IcedSize::new(physical_size.width, physical_size.height),
            window.scale_factor() as f32,
        );

        Self {
            renderer,
            theme: iced::Theme::TokyoNight,
            viewport,
            events: Vec::new(),
            cursor: IcedCursor::Unavailable,
            modifiers: winit::keyboard::ModifiersState::default(),
            ui_cache: ThreadLocal::new(Cache::new()),
        }
    }

    /// Convert a winit window event into iced events and buffer them for the
    /// next UI update.
    pub fn handle_window_event(
        &mut self,
        window: &Window,
        event: &winit::event::WindowEvent,
    ) {
        use iced_winit::conversion;

        match event {
            winit::event::WindowEvent::Resized(new_size) => {
                self.viewport = Viewport::with_physical_size(
                    IcedSize::new(new_size.width, new_size.height),
                    window.scale_factor() as f32,
                );
            }
            winit::event::WindowEvent::ScaleFactorChanged {
                scale_factor,
                ..
            } => {
                let size = self.viewport.physical_size();
                self.viewport = Viewport::with_physical_size(
                    IcedSize::new(size.width, size.height),
                    *scale_factor as f32,
                );
            }
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                self.cursor = IcedCursor::Available(
                    conversion::cursor_position(*position, self.viewport.scale_factor()),
                );
            }
            winit::event::WindowEvent::CursorLeft { .. } => {
                self.cursor = IcedCursor::Unavailable;
            }
            winit::event::WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers.state();
            }
            _ => {}
        }

        if let Some(event) = conversion::window_event(
            event.clone(),
            window.scale_factor() as f32,
            self.modifiers,
        ) {
            self.events.push(event);
        }
    }

    /// Run one iced UI update/draw cycle and render the result to the given
    /// surface texture view. Returns any messages produced by widgets.
    pub fn update_and_draw(
        &mut self,
        ui_state: &EditorUiState,
        focus_rename_input: &mut bool,
        focus_property_input: &mut bool,
        window_surface_view: &wgpu::TextureView,
    ) -> Vec<IcedMessage> {
        let element = build_editor_ui(ui_state);
        let bounds = self.viewport.logical_size();

        let cache = std::mem::replace(self.ui_cache.get_mut(), Cache::new());

        let mut ui = UserInterface::build(
            element,
            IcedSize::new(bounds.width, bounds.height),
            cache,
            &mut self.renderer,
        );

        // Focus the rename text input on the frame rename mode was entered so
        // the user can start typing immediately without an extra click.
        if *focus_rename_input {
            let mut focus_op = focusable::focus(RENAME_INPUT_ID);
            ui.operate(&self.renderer, &mut focus_op);
            *focus_rename_input = false;
        }
        // Focus the property value input when editing starts.
        if *focus_property_input {
            let mut focus_op = focusable::focus(PROPERTY_INPUT_ID);
            ui.operate(&self.renderer, &mut focus_op);
            *focus_property_input = false;
        }
        let mut messages = Vec::new();
        let events = std::mem::take(&mut self.events);
        let (_state, _statuses) = ui.update(
            &events,
            self.cursor,
            &mut self.renderer,
            &mut Null,
            &mut messages,
        );

        let style = iced_winit::core::renderer::Style {
            text_color: self.theme.palette().text,
        };

        ui.draw(
            &mut self.renderer,
            &self.theme,
            &style,
            self.cursor,
        );

        let cache = ui.into_cache();
        *self.ui_cache.get_mut() = cache;

        // iced_wgpu manages its own command encoder and queue submission.
        self.renderer.present(
            None,
            SCREEN_FORMAT,
            window_surface_view,
            &self.viewport,
        );

        messages
    }
}
