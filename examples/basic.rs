use bevy::prelude::*;
use bevy_iced::iced::widget::text;
use bevy_iced::iced::{
    Alignment, Style,
    widget::{Button, Column, Row, slider, text_input},
};
use bevy_iced::{IcedContext, IcedInterfacePlugin, IcedPlugin, IcedProgramSet, IcedSettings, iced};
use bevy_input::mouse::{MouseButtonInput, MouseWheel};
use rand::random as rng;

const NOTOSANS_REGULAR: iced::Font = iced::Font::with_name("Noto Sans");
const NOTOSANS_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");

#[derive(Event)]
pub enum UiMessage {}

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((
            IcedPlugin::<bevy_winit::WakeUp>::default()
                .fonts(vec![NOTOSANS_REGULAR_BYTES])
                .settings(iced::Settings {
                    default_font: NOTOSANS_REGULAR,
                    ..Default::default()
                }),
            IcedInterfacePlugin::<UiMessage>::default(),
            IcedInterfacePlugin::<UiMessage2>::default(),
        ))
        .add_event::<UiMessage>()
        .add_event::<UiMessage2>()
        .add_systems(Update, ui_system.in_set(IcedProgramSet::View))
        .insert_resource(UiData {
            scale: 50.0,
            text: "Welcome to Iced!".to_owned(),
        })
        .insert_resource(UiActive(false))
        .add_systems(Startup, build_program)
        .add_systems(
            Update,
            (
                tick,
                box_system.in_set(IcedProgramSet::Update),
                update_scale_factor,
                toggle_ui,
                ui_system_2.in_set(IcedProgramSet::View),
            ),
        )
        .run();
}

fn ui_system(time: Res<Time>, mut ctx: IcedContext<UiMessage>) {
    ctx.display(text(format!(
        "Hello Iced! Running for {:.2} seconds.",
        time.elapsed_secs()
    )));
}

#[derive(Resource)]
pub struct UiData {
    scale: f32,
    text: String,
}
#[derive(Resource, Deref, DerefMut)]
pub struct UiActive(bool);

#[derive(Clone, Event)]
enum UiMessage2 {
    BoxRequested,
    Scale(f32),
    Text(String),
}

fn build_program(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn tick(mut sprites: Query<&mut Sprite>, time: Res<Time>, data: Res<UiData>) {
    let scale = data.scale;
    for mut s in sprites.iter_mut() {
        s.custom_size = Some(Vec2::new(scale, scale) * time.elapsed_secs().sin().abs());
    }
}

fn box_system(
    mut commands: Commands,
    mut messages: EventReader<UiMessage2>,
    mut data: ResMut<UiData>,
    mut sprites: Query<&mut Sprite>,
) {
    let pos = (Vec3::new(rng(), rng(), 0.0) - Vec3::new(0.5, 0.5, 0.0)) * 300.0;
    for msg in messages.read() {
        match msg {
            UiMessage2::BoxRequested => {
                commands.spawn((
                    Sprite {
                        color: Color::srgba_u8(rng(), rng(), rng(), rng()),
                        custom_size: Some(Vec2::new(50.0, 50.0)),
                        ..Default::default()
                    },
                    Transform::from_translation(pos),
                ));
            }
            UiMessage2::Scale(new_scale) => {
                data.scale = *new_scale;
            }
            UiMessage2::Text(s) => {
                data.text.clone_from(s);
                for mut i in &mut sprites.iter_mut() {
                    i.color = Color::srgba_u8(rng(), rng(), rng(), rng());
                }
            }
        }
    }
}

fn update_scale_factor(
    mut wheel: EventReader<MouseWheel>,
    mut iced_settings: ResMut<IcedSettings>,
) {
    if wheel.is_empty() {
        return;
    }
    for event in wheel.read() {
        let scale_factor =
            (iced_settings.scale_factor.unwrap_or(1.0) + (event.y / 10.0) as f64).max(1.0);
        iced_settings.set_scale_factor(scale_factor);
    }
}

fn toggle_ui(mut buttons: EventReader<MouseButtonInput>, mut ui_active: ResMut<UiActive>) {
    for ev in buttons.read() {
        if ev.button == MouseButton::Right {
            **ui_active = !**ui_active;
        }
    }
}

fn ui_system_2(
    mut ctx: IcedContext<UiMessage2>,
    data: Res<UiData>,
    sprites: Query<(&Sprite,)>,
    ui_active: Res<UiActive>,
) {
    if !**ui_active {
        return;
    }

    let row = Row::new()
        .spacing(10)
        .align_y(Alignment::Center)
        .push(Button::new(text("Request box")).on_press(UiMessage2::BoxRequested))
        .push(text(format!(
            "{} boxes (amplitude: {})",
            sprites.iter().len(),
            data.scale
        )));
    let edit = text_input("", &data.text).on_input(UiMessage2::Text);
    let column = Column::new()
        .align_x(Alignment::Center)
        .spacing(10)
        .push(edit)
        .push(slider(0.0..=100.0, data.scale, UiMessage2::Scale))
        .push(row);
    ctx.display(column);
}
