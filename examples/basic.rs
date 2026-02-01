use bevy::prelude::*;
use bevy_iced::iced::widget::text;
use bevy_iced::{AppIcedExt, IcedContext, IcedPlugin, IcedSettings, iced};
use bevy_input::mouse::MouseWheel;

const NOTOSANS_REGULAR: iced::Font = iced::Font::with_name("Noto Sans");
const NOTOSANS_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");

#[derive(Message)]
pub enum UiMessage {}

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((IcedPlugin::default()
            .fonts(vec![NOTOSANS_REGULAR_BYTES])
            .settings(iced::Settings {
                default_font: NOTOSANS_REGULAR,
                ..Default::default()
            }),))
        .add_iced_interface::<UiMessage, _>(ui_system)
        .add_systems(Startup, build_program)
        .add_systems(Update, update_scale_factor)
        .run();
}

fn ui_system(time: Res<Time>, mut ctx: IcedContext<UiMessage>) {
    ctx.display(text(format!(
        "Hello Iced! Running for {:.2} seconds.",
        time.elapsed_secs()
    )));
}

fn build_program(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn update_scale_factor(
    mut wheel: MessageReader<MouseWheel>,
    mut iced_settings: ResMut<IcedSettings>,
) {
    if wheel.is_empty() {
        return;
    }
    for message in wheel.read() {
        let scale_factor = (iced_settings.scale_factor.unwrap_or(1.0) + message.y / 10.0).max(1.0);
        iced_settings.scale_factor = Some(scale_factor);
    }
}
