use bevy::prelude::*;
use bevy_iced::iced::widget::text;
use bevy_iced::{AppIcedExt, IcedContext, IcedPlugin, iced};
use bevy_input::ButtonState;
use bevy_input::keyboard::KeyboardInput;

const NOTOSANS_REGULAR: iced::Font = iced::Font::with_name("Noto Sans");
const NOTOSANS_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");

#[derive(Message)]
pub enum UiMessageFoo {}

#[derive(Message)]
pub enum UiMessageBar {}

#[derive(Resource, PartialEq, Eq)]
pub enum UiMode {
    Foo,
    Bar,
}

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(
            IcedPlugin::default()
                .fonts(vec![NOTOSANS_REGULAR_BYTES])
                .settings(iced::Settings {
                    default_font: NOTOSANS_REGULAR,
                    ..Default::default()
                }),
        )
        .add_iced_interface_when::<UiMessageFoo, _, _, _>(ui_system_foo, || {
            resource_equals(UiMode::Foo)
        })
        .add_iced_interface_when::<UiMessageBar, _, _, _>(ui_system_bar, || {
            resource_equals(UiMode::Bar)
        })
        .insert_resource(UiMode::Foo)
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(Update, toggle_system)
        .run();
}

fn toggle_system(mut keyboard: MessageReader<KeyboardInput>, mut active: ResMut<UiMode>) {
    for event in keyboard.read() {
        if event.key_code == KeyCode::Space && event.state == ButtonState::Pressed {
            *active = match *active {
                UiMode::Foo => UiMode::Bar,
                UiMode::Bar => UiMode::Foo,
            }
        }
    }
}

fn ui_system_foo(mut ctx: IcedContext<UiMessageFoo>) {
    ctx.display(text("Foo! Press space to switch to the Bar UI."));
}

fn ui_system_bar(mut ctx: IcedContext<UiMessageBar>) {
    ctx.display(text("Bar! Press space to switch to the Foo UI."));
}
