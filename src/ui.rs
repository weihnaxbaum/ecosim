use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
};

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (get_btn_input, get_text_input));
}

#[derive(Component)]
pub struct Focus;

#[derive(Component)]
pub struct Focusable {
    pub order: u8,
}

#[derive(Component)]
pub struct TextInput {
    pub on_submit: fn(&str, Commands) -> bool,
}

#[derive(Component)]
pub struct ActionButton {
    pub on_press: fn(Commands),
}

fn get_btn_input(
    mut kb: MessageReader<KeyboardInput>,
    btn: Single<(Entity, &ActionButton, &Focusable), With<Focus>>,
    focusable: Query<(Entity, &Focusable), Without<Focus>>,
    mut commands: Commands,
) {
    let (btn_e, btn, current_focus) = btn.into_inner();
    for ki in kb.read() {
        if ki.state == ButtonState::Released {
            continue;
        }
        if ki.key_code == KeyCode::Enter {
            (btn.on_press)(commands.reborrow());
            return;
        } else if ki.key_code == KeyCode::ArrowUp {
            let next = current_focus.order.saturating_sub(1);
            for (e, focusable) in &focusable {
                if focusable.order == next {
                    commands.entity(btn_e).remove::<Focus>();
                    commands.entity(e).insert(Focus);
                    return;
                }
            }
            return;
        } else if ki.key_code == KeyCode::ArrowDown {
            let next = current_focus.order.saturating_add(1);
            for (e, focusable) in &focusable {
                if focusable.order == next {
                    commands.entity(btn_e).remove::<Focus>();
                    commands.entity(e).insert(Focus);
                    return;
                }
            }
            return;
        }
    }
}

fn get_text_input(
    mut kb: MessageReader<KeyboardInput>,
    text_input: Single<(Entity, &mut Text2d, &TextInput, &Focusable), With<Focus>>,
    focusable: Query<(Entity, &Focusable), Without<Focus>>,
    mut commands: Commands,
) {
    let (in_e, mut text2d, text_input, current_focus) = text_input.into_inner();
    for ki in kb.read() {
        if ki.state == ButtonState::Released {
            continue;
        }

        if ki.key_code == KeyCode::Enter {
            if !(text_input.on_submit)(&text2d.0, commands.reborrow()) {
                text2d.clear();
            }
            return;
        } else if ki.key_code == KeyCode::Backspace {
            text2d.pop();
        } else if ki.key_code == KeyCode::ArrowUp {
            if !(text_input.on_submit)(&text2d.0, commands.reborrow()) {
                text2d.clear();
            }
            let next = current_focus.order.saturating_sub(1);
            for (e, focusable) in &focusable {
                if focusable.order == next {
                    commands.entity(in_e).remove::<Focus>();
                    commands.entity(e).insert(Focus);
                    return;
                }
            }
            return;
        } else if ki.key_code == KeyCode::ArrowDown {
            if !(text_input.on_submit)(&text2d.0, commands.reborrow()) {
                text2d.clear();
            }
            let next = current_focus.order.saturating_add(1);
            for (e, focusable) in &focusable {
                if focusable.order == next {
                    commands.entity(in_e).remove::<Focus>();
                    commands.entity(e).insert(Focus);
                    return;
                }
            }
            return;
        } else if let Some(text) = &ki.text {
            text2d.push_str(text);
        }
    }
}
