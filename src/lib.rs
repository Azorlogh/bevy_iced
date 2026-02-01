//! # Use Iced UI programs in your Bevy application
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_iced::iced::widget::text;
//! use bevy_iced::{IcedContext, IcedPlugin};
//!
//! #[derive(Event)]
//! pub enum UiMessage {}
//!
//! pub fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(IcedPlugin::default())
//!         .add_event::<UiMessage>()
//!         .add_systems(Update, ui_system)
//!         .run();
//! }
//!
//! fn ui_system(time: Res<Time>, mut ctx: IcedContext<UiMessage>) {
//!     ctx.display(text(format!(
//!         "Hello Iced! Running for {:.2} seconds.",
//!         time.elapsed_seconds()
//!     )));
//! }
//! ```

#![deny(unsafe_code)]
#![deny(missing_docs)]

use std::borrow::Cow;
use std::marker::PhantomData;

use bevy_app::prelude::*;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::BoxedCondition;
use bevy_ecs::system::{SystemId, SystemParam};
use bevy_render::render_graph::RenderGraph;
use bevy_render::renderer::{RenderAdapter, RenderDevice, RenderQueue, render_system};
use bevy_render::{Render, RenderApp};
use bevy_render::{RenderSystems, prelude::*};
use bevy_window::{PrimaryWindow, Window};
use cfg_if::cfg_if;
use iced_core::Theme;
use iced_core::mouse::Cursor;
use iced_graphics::Shell;
use iced_runtime::user_interface::UserInterface;
use iced_widget::graphics::Viewport;

use redraw_requestor::{IcedRedrawRequest, RedrawRequestor};
use render::IcedViewport;
use systems::{IcedCursor, IcedEventQueue};

/// Basic re-exports for all Iced-related stuff.
///
/// This module attempts to emulate the `iced` package's API
/// as much as possible.
pub mod iced;

mod conversions;
mod redraw_requestor;
mod render;
mod systems;
mod utils;

pub use systems::IcedInterface;

/// The default renderer.
pub type Renderer = iced_wgpu::Renderer;
/// The default element.
pub type Element<'a, Msg> = iced::Element<'a, Msg, iced::Theme, Renderer>;

/// Plugin for an iced UI with a specific Message.
/// Multiple UIs can be added to the same app using [AppIcedExt::add_iced_interface] and [AppIcedExt::add_iced_interface_when].
pub struct IcedInterfacePlugin<Message: 'static>(
    SystemId,
    Option<Box<dyn Fn() -> BoxedCondition + Sync + Send>>,
    PhantomData<Message>,
);

/// Used internally to track of why the `view` was requested.
#[derive(Resource)]
pub enum IcedRunCause<Msg> {
    /// Draw the UI
    Draw(PhantomData<Msg>),
    /// Update the UI
    Update,
}

/// Extension trait to add user interfaces to the an [App].
pub trait AppIcedExt {
    /// Add a UI with the given message and view function.
    ///
    /// # Examples
    ///
    /// ```
    /// app.add_iced_interface::<UiMessage, _>(view);
    /// ```
    fn add_iced_interface<Msg: bevy_ecs::message::Message, M>(
        &mut self,
        f: impl IntoSystem<(), (), M> + 'static,
    ) -> &mut Self;

    /// Add a UI with the given message and view function, which only runs when the given condition is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// app.add_iced_interface_when::<UiMessage, _, _, _>(view, || in_state(InMenu));
    /// ```
    fn add_iced_interface_when<Msg: bevy_ecs::message::Message, M, M2, C: SystemCondition<M2>>(
        &mut self,
        system: impl IntoSystem<(), (), M> + 'static,
        condition: impl Fn() -> C + Sync + Send + 'static,
    ) -> &mut Self;
}
impl AppIcedExt for App {
    fn add_iced_interface<Msg: bevy_ecs::message::Message, M>(
        &mut self,
        system: impl IntoSystem<(), (), M> + 'static,
    ) -> &mut Self {
        let system_id = self.register_system(system);
        self.add_plugins(IcedInterfacePlugin(
            system_id,
            // Arc::new(Mutex::new(None)),
            None,
            PhantomData::<Msg>,
        ));
        self
    }
    fn add_iced_interface_when<Msg: bevy_ecs::message::Message, M, M2, C: SystemCondition<M2>>(
        &mut self,
        system: impl IntoSystem<(), (), M> + 'static,
        condition: impl Fn() -> C + Sync + Send + 'static,
    ) -> &mut Self {
        let system_id = self.register_system(system);
        self.add_plugins(IcedInterfacePlugin(
            system_id,
            // Arc::new(Mutex::new(Some(
            //     Box::new(IntoSystem::into_system(condition)) as BoxedCondition,
            // ))),
            Some(Box::new(move || {
                Box::new(IntoSystem::into_system(condition())) as BoxedCondition
            })),
            PhantomData::<Msg>,
        ));
        self
    }
}

// pub struct IcedSystemId(SystemId<In<IcedRunnerContext<Msg>>>);

impl<Message: bevy_ecs::message::Message> Plugin for IcedInterfacePlugin<Message> {
    fn build(&self, app: &mut App) {
        let app = app
            .add_message::<Message>()
            .insert_non_send_resource::<IcedInterface<Message>>(IcedInterface::default());

        let mut update = (
            systems::iced_update::<Message>,
            systems::iced_update_run::<Message>(self.0),
        )
            .chain();
        if let Some(condition) = &self.1 {
            update.run_if_dyn(condition());
        }
        app.add_systems(PreUpdate, update.in_set(IcedUpdateSet));

        let mut draw = systems::iced_draw::<Message>(self.0);
        if let Some(condition) = &self.1 {
            draw.run_if_dyn(condition());
        }
        app.add_systems(Update, draw.in_set(IcedProgramSet::View));
    }
}

/// The main feature of `bevy_iced`.
/// Add this to your [`App`] by calling `app.add_plugin(bevy_iced::IcedPlugin::<Message>::default())`.
///
/// `Message` is the type of of message that is produced by the UI.
/// `WinitUserEvent` is the UserEvent type for the Winit event loop.
/// If you are not overriding this type in the `WinitPlugin`, you don't need to set this manually.
pub struct IcedPlugin {
    /// Settings
    pub settings: iced::Settings,
    /// Fonts
    pub fonts: Vec<&'static [u8]>,
}

impl Default for IcedPlugin {
    fn default() -> Self {
        Self {
            settings: Default::default(),
            fonts: Default::default(),
        }
    }
}

impl IcedPlugin {
    /// Set the Iced settings.
    pub fn settings(mut self, settings: iced::Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Set the fonts to preload in Iced.
    pub fn fonts(mut self, fonts: Vec<&'static [u8]>) -> Self {
        self.fonts = fonts;
        self
    }
}

/// Update
#[derive(Clone, Debug, PartialEq, Eq, Hash, SystemSet)]
pub struct IcedUpdateSet;

impl Plugin for IcedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (systems::process_input, render::update_viewport)
                .before(IcedUpdateSet)
                .after(bevy_input::InputSystems),
        )
        .init_resource::<DidDraw>()
        .init_resource::<IcedSettings>()
        .init_resource::<IcedEventQueue>()
        .init_resource::<IcedCursor>()
        .init_resource::<IcedRedrawRequest>()
        .configure_sets(Update, IcedProgramSet::View.after(IcedProgramSet::Update));
    }

    fn finish(&self, app: &mut App) {
        let default_viewport = Viewport::with_physical_size(iced_core::Size::new(1600, 900), 1.0);
        let default_viewport = IcedViewport(default_viewport);
        let iced_resource: IcedResource = IcedProps::new(app, self).into();

        app.insert_resource(default_viewport.clone());
        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                app.insert_non_send_resource(iced_resource.clone());
            } else {
                app.insert_resource(iced_resource.clone());
            }
        }

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .insert_resource(default_viewport)
            .add_systems(ExtractSchedule, render::extract_iced_data)
            .add_systems(
                Render,
                render::recall_staging_belt
                    .after(render_system)
                    .in_set(RenderSystems::Render),
            );
        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                render_app.world_mut().insert_non_send_resource(iced_resource);
            } else {
                render_app.world_mut().insert_resource(iced_resource);
            }
        }
        setup_pipeline(&mut render_app.world_mut().get_resource_mut().unwrap());
    }
}

/// SystemSet for specifying which systems perform view and update logic.
#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Clone)]
pub enum IcedProgramSet {
    /// The set of systems that update the UI state.
    Update,
    /// The system that renders the UI.
    View,
}

struct IcedProps {
    renderer: Renderer,
}

impl IcedProps {
    fn new(app: &App, config: &IcedPlugin) -> Self {
        let render_world = &app.sub_app(RenderApp).world();
        let device = render_world
            .get_resource::<RenderDevice>()
            .unwrap()
            .wgpu_device();
        let queue: &iced_wgpu::wgpu::Queue = render_world.get_resource::<RenderQueue>().unwrap();
        let adapter = render_world.get_resource::<RenderAdapter>().unwrap();
        let engine = iced_wgpu::Engine::new(
            adapter,
            device.clone(),
            queue.clone(),
            render::TEXTURE_FMT,
            Some(iced_wgpu::graphics::Antialiasing::MSAAx4),
            Shell::headless(),
        );

        for &font in &config.fonts {
            iced_graphics::text::font_system()
                .write()
                .expect("write lock on global FontSystem")
                .load_font(Cow::from(font));
        }

        Self {
            renderer: iced_wgpu::Renderer::new(
                engine,
                config.settings.default_font,
                config.settings.default_text_size,
            ),
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(private_interfaces)]
mod iced_resource {
    use super::*;

    use std::cell::{RefCell, RefMut};
    use std::rc::Rc;

    #[derive(Clone)]
    pub struct IcedResource(Rc<RefCell<IcedProps>>);

    impl IcedResource {
        pub fn lock(&self) -> RefMut<IcedProps> {
            self.0.borrow_mut()
        }
    }

    impl From<IcedProps> for IcedResource {
        fn from(value: IcedProps) -> Self {
            Self(Rc::new(RefCell::new(value)))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(private_interfaces)]
mod iced_resource {
    use super::*;

    use std::sync::{Arc, Mutex, MutexGuard};

    #[derive(Resource, Clone)]
    pub struct IcedResource(pub Arc<Mutex<IcedProps>>);

    impl IcedResource {
        pub fn lock(&self) -> MutexGuard<'_, IcedProps> {
            self.0.lock().unwrap()
        }
    }

    impl From<IcedProps> for IcedResource {
        fn from(value: IcedProps) -> Self {
            Self(Arc::new(Mutex::new(value)))
        }
    }
}
use iced_resource::IcedResource;

fn setup_pipeline(graph: &mut RenderGraph) {
    graph.add_node(render::IcedPass, render::IcedNode);

    graph.add_node_edge(bevy_render::graph::CameraDriverLabel, render::IcedPass);
}

/// Settings used to independently customize Iced rendering.
#[derive(Clone, Resource)]
pub struct IcedSettings {
    /// The scale factor to use for rendering Iced elements.
    /// Setting this to `None` defaults to using the `Window`s scale factor.
    pub scale_factor: Option<f32>,
    /// The theme to use for rendering Iced elements.
    pub theme: Theme,
    /// The style to use for rendering Iced elements.
    pub style: iced::Style,
}

impl Default for IcedSettings {
    fn default() -> Self {
        Self {
            scale_factor: None,
            theme: Theme::Dark,
            style: iced::Style {
                text_color: iced_core::Color::WHITE,
            },
        }
    }
}

// An atomic flag for updating the draw state.
#[derive(Resource, Deref, DerefMut, Default)]
pub(crate) struct DidDraw(std::sync::atomic::AtomicBool);

/// The context for interacting with Iced. Add this as a parameter to your system.
/// ```ignore
/// fn ui_system(..., mut ctx: IcedContext<UiMessage>) {
///     let element = ...; // Build your element
///     ctx.display(element);
/// }
/// ```
///
/// `IcedContext<T>` requires an event system to be defined in the [`App`].
/// Do so by invoking `app.add_event::<T>()` when constructing your App.
#[derive(SystemParam)]
pub struct IcedContext<'w, 's, Message>
where
    Message: bevy_ecs::message::Message,
{
    viewport: Res<'w, IcedViewport>,
    #[cfg(target_arch = "wasm32")]
    props: NonSend<'w, IcedResource>,
    #[cfg(not(target_arch = "wasm32"))]
    props: Res<'w, IcedResource>,
    settings: Res<'w, IcedSettings>,
    did_draw: ResMut<'w, DidDraw>,
    events: ResMut<'w, IcedEventQueue>,
    touches: ResMut<'w, bevy_input::prelude::Touches>,
    // ui: NonSendMut<'w, Option<UserInterface<'static, Message, Theme, Renderer>>>,
    interface: NonSendMut<'w, IcedInterface<Message>>,
    cause: ResMut<'w, IcedRunCause<Message>>,
    cursor: ResMut<'w, IcedCursor>,
    message_writer: MessageWriter<'w, Message>,
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    redraw_requestor: RedrawRequestor<'w, 's>,
}

// pub struct UserInterfaceCache(iced_runtime::user_interface::Cache);

impl<M> IcedContext<'_, '_, M>
where
    M: bevy_ecs::message::Message,
{
    /// Display an [`Element`] to the screen.
    pub fn display<'a>(&mut self, element: impl Into<iced_core::Element<'a, M, Theme, Renderer>>) {
        match *self.cause {
            IcedRunCause::Draw(_) => {
                let &mut IcedProps {
                    ref mut renderer, ..
                } = &mut *self.props.lock();
                let bounds = self.viewport.logical_size();

                // Rebuild the UI using the new element.
                let cache = std::mem::take(&mut self.interface.cache);
                let mut ui = UserInterface::build(element.into(), bounds, cache, renderer);

                // Run the UI update function with a single redraw request.
                // This is necessary to account for widget state that depends on external state (like time).
                let mut messages = Vec::<M>::new();
                let events = [iced_core::Event::Window(
                    iced_core::window::Event::RedrawRequested(iced_core::time::Instant::now()),
                )];
                let (state, _event_statuses) = ui.update(
                    events.as_slice(),
                    **self.cursor,
                    renderer,
                    &mut iced_core::clipboard::Null,
                    &mut messages,
                );
                self.redraw_requestor.finish(state);
                self.message_writer.write_batch(messages);

                // Draw the UI.
                ui.draw(
                    renderer,
                    &self.settings.theme,
                    &self.settings.style,
                    **self.cursor,
                );
                self.interface.cache = ui.into_cache();
                self.did_draw
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            IcedRunCause::Update => {
                let bounds = self.viewport.logical_size();
                let cache = std::mem::take(&mut self.interface.cache);
                let &mut IcedProps {
                    ref mut renderer, ..
                } = &mut *self.props.lock();
                let mut ui = UserInterface::build(element, bounds, cache, renderer);
                let mut messages = Vec::<M>::new();
                *self.cursor = IcedCursor({
                    let window = self.window.single().unwrap();
                    match window.cursor_position() {
                        Some(position) => Cursor::Available(utils::process_cursor_position(
                            position, bounds, window,
                        )),
                        None => utils::process_touch_input(&self.touches, &self.events)
                            .map(Cursor::Available)
                            .unwrap_or(Cursor::Unavailable),
                    }
                });
                let (_state, _event_statuses) = ui.update(
                    self.events.as_slice(),
                    **self.cursor,
                    renderer,
                    &mut iced_core::clipboard::Null,
                    &mut messages,
                );
                self.events.clear();

                self.interface.cache = ui.into_cache();
                self.message_writer.write_batch(messages);
            }
        }
    }
}
