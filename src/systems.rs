use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_input::prelude::*;
use bevy_input::touch::TouchInput;
use bevy_input::{
    ButtonState,
    keyboard::KeyboardInput,
    mouse::{MouseButtonInput, MouseWheel},
};
use bevy_tasks::Task;
use bevy_tasks::prelude::*;
use bevy_window::prelude::*;
use bevy_window::{PrimaryWindow, WindowFocused};
use bevy_winit::{EventLoopProxyWrapper, WakeUp};
use cfg_if::cfg_if;
use iced_core::time::Instant;
use iced_core::window::Event as IcedWindowEvent;
use iced_core::{
    Event as IcedEvent, Point, Theme, keyboard,
    mouse::{self, Cursor},
};
use iced_runtime::UserInterface;

use crate::{
    IcedProps, Renderer, conversions, iced_resource::IcedResource, render::IcedViewport, utils,
};

#[derive(Resource, Deref, DerefMut, Default)]
pub struct IcedEventQueue(Vec<iced_core::Event>);

#[derive(SystemParam)]
pub struct InputEvents<'w, 's> {
    cursor_entered: EventReader<'w, 's, CursorEntered>,
    cursor_left: EventReader<'w, 's, CursorLeft>,
    cursor: EventReader<'w, 's, CursorMoved>,
    mouse_button: EventReader<'w, 's, MouseButtonInput>,
    mouse_wheel: EventReader<'w, 's, MouseWheel>,
    keyboard_input: EventReader<'w, 's, KeyboardInput>,
    touch_input: EventReader<'w, 's, TouchInput>,
    window_focused: EventReader<'w, 's, WindowFocused>,
}

fn compute_modifiers(input_map: &ButtonInput<KeyCode>) -> keyboard::Modifiers {
    let mut modifiers = keyboard::Modifiers::default();
    if input_map.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        modifiers |= keyboard::Modifiers::CTRL;
    }
    if input_map.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        modifiers |= keyboard::Modifiers::SHIFT;
    }
    if input_map.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]) {
        modifiers |= keyboard::Modifiers::ALT;
    }
    if input_map.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight]) {
        modifiers |= keyboard::Modifiers::LOGO;
    }
    modifiers
}

pub fn process_input(
    mut events: InputEvents,
    mut event_queue: ResMut<IcedEventQueue>,
    input_map: Res<ButtonInput<KeyCode>>,
) {
    event_queue.clear();

    for ev in events.cursor.read() {
        event_queue.push(IcedEvent::Mouse(mouse::Event::CursorMoved {
            position: Point::new(ev.position.x, ev.position.y),
        }));
    }

    for ev in events.mouse_button.read() {
        let button = conversions::mouse_button(ev.button);
        event_queue.push(IcedEvent::Mouse(match ev.state {
            ButtonState::Pressed => iced_core::mouse::Event::ButtonPressed(button),
            ButtonState::Released => iced_core::mouse::Event::ButtonReleased(button),
        }));
    }

    for _ev in events.cursor_entered.read() {
        event_queue.push(IcedEvent::Mouse(iced_core::mouse::Event::CursorEntered));
    }

    for _ev in events.cursor_left.read() {
        event_queue.push(IcedEvent::Mouse(iced_core::mouse::Event::CursorLeft));
    }

    for ev in events.mouse_wheel.read() {
        event_queue.push(IcedEvent::Mouse(iced_core::mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: ev.x, y: ev.y },
        }));
    }

    let modifiers = compute_modifiers(&input_map);

    for ev in events.keyboard_input.read() {
        use keyboard::Event::*;
        let event = match ev.key_code {
            KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight => ModifiersChanged(modifiers),
            _ => {
                let key = conversions::key(&ev.logical_key);
                let physical_key = conversions::key_code(ev.key_code);
                if ev.state.is_pressed() {
                    KeyPressed {
                        // NOTE: This is supposed to be the "unmodified" key, but we don't get it from bevy events
                        key: key.clone(),
                        text: conversions::key_text(&key),
                        physical_key,
                        modified_key: key,
                        modifiers,
                        // NOTE: This is a winit thing we don't get from bevy events
                        location: keyboard::Location::Standard,
                    }
                } else {
                    KeyReleased {
                        key: key.clone(),
                        modified_key: key,
                        physical_key,
                        modifiers,
                        // NOTE: This is a winit thing we don't get from bevy events
                        location: keyboard::Location::Standard,
                    }
                }
            }
        };

        event_queue.push(IcedEvent::Keyboard(event));
    }

    for ev in events.touch_input.read() {
        event_queue.push(IcedEvent::Touch(conversions::touch_event(ev)));
    }

    for ev in events.window_focused.read() {
        event_queue.push(IcedEvent::Window(if ev.focused {
            IcedWindowEvent::Focused
        } else {
            IcedWindowEvent::Unfocused
        }));
    }

    event_queue.push(IcedEvent::Window(IcedWindowEvent::RedrawRequested(
        Instant::now(),
    )));
}

#[derive(Resource, Deref, DerefMut, Default)]
pub struct IcedCursor(Cursor);

/// A trait for types that can be used to request a redraw.
pub trait RedrawRequest: Event + Send + Sync + 'static {
    /// The event that should be sent to request a redraw.
    const REDRAW_REQUEST: Self;
}

impl RedrawRequest for WakeUp {
    const REDRAW_REQUEST: Self = Self;
}

#[derive(SystemParam)]
pub struct RedrawRequestor<'w, 's, U: RedrawRequest> {
    task: Local<'s, Option<Task<()>>>,
    event_loop_proxy: Res<'w, EventLoopProxyWrapper<U>>,
}

impl<E: RedrawRequest> RedrawRequestor<'_, '_, E> {
    fn request_redraw(&mut self) {
        self.task.take();
        let _ = self.event_loop_proxy.send_event(E::REDRAW_REQUEST);
    }

    fn request_redraw_at(&mut self, instant: Instant) {
        let event_loop_proxy = self.event_loop_proxy.clone();
        let f = async move {
            cfg_if! {
                if #[cfg(target_arch = "wasm32")] {
                    gloo_timers::future::TimeoutFuture::new(
                        instant
                            .saturating_duration_since(Instant::now())
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
            let _ = event_loop_proxy.send_event(E::REDRAW_REQUEST);
        };
        #[cfg(all(not(target_arch = "wasm32"), feature = "tokio"))]
        let f = async_compat::Compat::new(f);
        let task = IoTaskPool::get().spawn(f);
        *self.task = Some(task);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn iced_update<M: bevy_ecs::event::Event, U: RedrawRequest>(
    viewport: Res<IcedViewport>,
    #[cfg(target_arch = "wasm32")] props: NonSend<IcedResource>,
    #[cfg(not(target_arch = "wasm32"))] props: Res<IcedResource>,
    windows: Query<&mut Window, With<PrimaryWindow>>,
    mut events: ResMut<IcedEventQueue>,
    touches: Res<Touches>,
    mut ui: NonSendMut<Option<UserInterface<'static, M, Theme, Renderer>>>,
    mut message_writer: EventWriter<M>,
    mut cursor: ResMut<IcedCursor>,
    mut redraw_requestor: RedrawRequestor<U>,
) {
    let bounds = viewport.logical_size();
    let &mut IcedProps {
        ref mut renderer, ..
    } = &mut *props.lock();
    *cursor = IcedCursor({
        let window = windows.single().unwrap();
        match window.cursor_position() {
            Some(position) => {
                Cursor::Available(utils::process_cursor_position(position, bounds, window))
            }
            None => utils::process_touch_input(&touches, &events)
                .map(Cursor::Available)
                .unwrap_or(Cursor::Unavailable),
        }
    });
    let Some(ui) = ui.as_mut() else { return };

    let mut messages = Vec::<M>::new();
    let (state, _event_statuses) = ui.update(
        events.as_slice(),
        **cursor,
        renderer,
        &mut iced_core::clipboard::Null,
        &mut messages,
    );
    events.clear();
    message_writer.write_batch(messages);

    {
        use iced_core::window::RedrawRequest;
        use iced_runtime::user_interface::State;
        match state {
            State::Updated {
                redraw_request,
                input_method: _,
            } => match redraw_request {
                RedrawRequest::NextFrame => redraw_requestor.request_redraw(),
                RedrawRequest::At(instant) => redraw_requestor.request_redraw_at(instant),
                RedrawRequest::Wait => {}
            },
            State::Outdated => {}
        }
    }
}
