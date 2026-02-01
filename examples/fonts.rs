use bevy::prelude::*;
use bevy_iced::iced::{
    Font,
    widget::{column, text},
};
use bevy_iced::{AppIcedExt, IcedContext, IcedPlugin, iced};

const ALPHAPROTA_FONT: Font = Font::with_name("Alpha Prota");
const ALPHAPROTA_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/AlphaProta.ttf");
const NOTOSANS_REGULAR: iced::Font = iced::Font::with_name("Noto Sans");
const NOTOSANS_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");

#[derive(Message)]
pub enum UiMessage {}

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(
            IcedPlugin::default()
                .fonts(vec![ALPHAPROTA_FONT_BYTES, NOTOSANS_REGULAR_BYTES])
                .settings(iced::Settings {
                    default_text_size: 40.0.into(),
                    default_font: NOTOSANS_REGULAR,
                    ..Default::default()
                }),
        )
        .add_iced_interface::<UiMessage, _>(ui_system)
        .run();
}

fn ui_system(mut ctx: IcedContext<UiMessage>) {
    ctx.display(column!(
        text("I am the default font".to_string()),
        text("I am another font".to_string()).font(ALPHAPROTA_FONT)
    ));
}
