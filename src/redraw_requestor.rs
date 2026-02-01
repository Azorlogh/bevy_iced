use std::mem;

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_tasks::Task;
use bevy_tasks::prelude::*;
use bevy_winit::EventLoopProxyWrapper;
use bevy_winit::WinitUserEvent;
use cfg_if::cfg_if;
use iced_core::window::RedrawRequest;
use iced_runtime::user_interface::State;

#[derive(Resource, PartialEq, Eq, PartialOrd, Ord)]
pub struct IcedRedrawRequest(RedrawRequest);

impl Default for IcedRedrawRequest {
    fn default() -> Self {
        Self(RedrawRequest::Wait)
    }
}

impl From<State> for IcedRedrawRequest {
    fn from(state: State) -> Self {
        match state {
            State::Updated {
                redraw_request,
                input_method: _,
                ..
            } => Self(redraw_request),
            State::Outdated => Self(RedrawRequest::Wait),
        }
    }
}

impl IcedRedrawRequest {
    pub fn update(&mut self, state: State) {
        self.0 = self.0.min(Self::from(state).0);
    }

    fn take(&mut self) -> RedrawRequest {
        mem::replace(&mut self.0, RedrawRequest::Wait)
    }
}

#[derive(SystemParam)]
pub struct RedrawRequestor<'w, 's> {
    pub task: Local<'s, Option<Task<()>>>,
    pub event_loop_proxy: Res<'w, EventLoopProxyWrapper>,
    pub redraw_request: ResMut<'w, IcedRedrawRequest>,
}

impl RedrawRequestor<'_, '_> {
    pub fn finish(&mut self, state: State) {
        self.redraw_request.update(state);

        self.task.take();
        let redraw_request = self.redraw_request.take();
        match redraw_request {
            RedrawRequest::NextFrame => {
                let _ = self.event_loop_proxy.send_event(WinitUserEvent::WakeUp);
            }
            RedrawRequest::At(instant) => {
                let event_loop_proxy = self.event_loop_proxy.clone();
                let f = async move {
                    cfg_if! {
                        if #[cfg(target_arch = "wasm32")] {
                            gloo_timers::future::TimeoutFuture::new(
                                instant
                                    .saturating_duration_since(iced_core::time::Instant::now())
                                    .as_millis().min(u32::MAX as _) as u32
                            ).await;
                        } else if #[cfg(feature = "tokio")] {
                            tokio::time::sleep_until(instant.into()).await;
                        } else if #[cfg(feature = "smol")] {
                            async_io::Timer::at(instant).await;
                        } else {
                            compile_error!("Either the `tokio` or `smol` feature must be enabled");
                        }
                    }
                    let _ = event_loop_proxy.send_event(WinitUserEvent::WakeUp);
                };
                #[cfg(all(not(target_arch = "wasm32"), feature = "tokio"))]
                let f = async_compat::Compat::new(f);
                let task = IoTaskPool::get().spawn(f);
                *self.task = Some(task);
            }
            RedrawRequest::Wait => {}
        }
    }
}
