use std::marker::PhantomData;
use std::rc::Rc;
use std::{cell::RefCell, sync::Arc};

use crate::render::IcedNode;
use bevy::prelude::NonSendMut;
use bevy::render::render_graph::RenderGraph;
use bevy::{
    prelude::{App, Plugin, World},
    render::{
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::BevyDefault,
        view::ExtractedWindows,
        RenderApp,
    },
};
use iced_native::futures::executor::LocalPool;
use iced_native::futures::task::SpawnExt;
use iced_native::{Event as IcedEvent, Clipboard};
use iced_native::{program, Debug, Program, Size};
use iced_wgpu::{
    wgpu::{self, util::StagingBelt, CommandEncoderDescriptor},
    Viewport,
};
use render::{IcedPipeline, IcedRenderData};

pub type IcedState<T> = Arc<RefCell<program::State<T>>>;

mod conversions;
mod render;
mod systems;
pub struct IcedPlugin;

impl Plugin for IcedPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_system(systems::process_input)
            .insert_resource(Vec::<IcedEvent>::new());

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .insert_non_send_resource(RefCell::new(Vec::<DrawFn>::new()))
            .insert_non_send_resource(Vec::<UpdateFn>::new());
        render_app.init_resource::<render::IcedPipeline>();
        setup_pipeline(&mut render_app.world.get_resource_mut().unwrap());
    }
}

type UpdateFn = Box<dyn FnMut(&mut World, &Viewport, Option<&iced_native::Event>)>;
type DrawFn = Box<dyn FnMut(&World, &mut RenderContext, &Viewport, &mut render::IcedRenderData)>;

struct IcedProgramData<T> {
    renderer: iced_wgpu::Renderer,
    debug: iced_native::Debug,
    _phantom: PhantomData<T>
}

pub trait IcedAppExtensions {
    fn insert_program<M, T: Program<Renderer = iced_wgpu::Renderer, Message = M> + 'static>(
        &mut self,
        program: T,
    ) -> &mut Self;
}

impl IcedAppExtensions for App {
    fn insert_program<M, T: Program<Renderer = iced_wgpu::Renderer, Message = M> + 'static>(
        &mut self,
        program: T,
    ) -> &mut Self {
        let device = self
            .sub_app(RenderApp)
            .world
            .get_resource::<RenderDevice>()
            .unwrap()
            .wgpu_device();
        let format = wgpu::TextureFormat::bevy_default();
        let mut renderer =
            iced_wgpu::Renderer::new(iced_wgpu::Backend::new(device, Default::default(), format));
        let viewport = Viewport::with_physical_size(Size::new(1600, 900), 1.0);
        let mut debug = Debug::new();
        let mut clipboard = iced_native::clipboard::Null;
        let program =
            program::State::new(program, viewport.logical_size(), &mut renderer, &mut debug);
        
        let update_data = Arc::new(IcedProgramData::<T> {
            renderer,
            debug,
            _phantom: Default::default(),
        });
        let draw_data = update_data.clone();
        self.insert_non_send_resource(update_data.clone());

        self.add_system(move |mut state: NonSendMut<program::State<T>>, mut data: NonSendMut<Arc<IcedProgramData<T>>>| {
            // println!("running update system");
            let IcedProgramData::<T> { renderer, debug, _phantom } = unsafe {
                get_rc_mut(&mut *data)
            };

            // if !state.is_queue_empty() {
                state.update(
                    viewport.logical_size(),
                    iced_native::Point { x: 0.0, y: 0.0 },
                    renderer,
                    &mut clipboard,
                    debug,
                );
            // }
        });
        
        let update_fn: UpdateFn = Box::new(move |world: &mut World, viewport: &Viewport, event: Option<&iced_native::Event>| {
            println!("running update");
            
            let mut state = world
                .get_non_send_resource_mut::<program::State<T>>()
                .unwrap();
            if let Some(ev) = event {
                state.queue_event(ev.clone());
            }
            
        });
        self.sub_app_mut(RenderApp)
            .world
            .get_non_send_resource_mut::<Vec<UpdateFn>>()
            .unwrap()
            .push(update_fn);

        let draw_fn: DrawFn = Box::new(move |world: &World, ctx: &mut RenderContext, viewport: &Viewport, data: &mut IcedRenderData| {
            // println!("running draw");
            let IcedProgramData::<T> { renderer, debug, _phantom } = unsafe {
                get_rc_mut(&draw_data)
            };

            let device = ctx.render_device.wgpu_device();

            renderer.with_primitives(|backend, primitive| {
                backend.present(
                    device,
                    data.staging_belt,
                    data.encoder,
                    data.view,
                    primitive,
                    &viewport,
                    &debug.overlay(),
                );
            });
        });

        self.sub_app_mut(RenderApp)
            .world
            .get_non_send_resource_mut::<RefCell<Vec<DrawFn>>>()
            .unwrap()
            .borrow_mut()
            .push(draw_fn);

        self.insert_non_send_resource(program);
        self
    }
}

pub fn setup_pipeline(graph: &mut RenderGraph) {
    graph.add_node(render::ICED_PASS, IcedNode::new());

    graph
        .add_node_edge(
            bevy::core_pipeline::node::MAIN_PASS_DRIVER,
            render::ICED_PASS,
        )
        .unwrap();
}

pub unsafe fn get_rc_mut<'a, T>(rc: &'a Arc<T>) -> &'a mut T {
    let data = &**rc as *const T as *mut T;
    &mut *data
}