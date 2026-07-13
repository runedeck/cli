use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use super::app::App;

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    if app.modal_blocks_mouse() {
        return;
    }
    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            app.clear_toast();
            app.mouse_click(mouse.column, mouse.row);
        }
        MouseEventKind::ScrollDown => app.mouse_scroll(mouse.column, mouse.row, true),
        MouseEventKind::ScrollUp => app.mouse_scroll(mouse.column, mouse.row, false),
        _ => {}
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    app.clear_toast();
    if key.code != KeyCode::Char('d') || !app.is_comment_navigator_focused() {
        app.disarm_comment_delete();
    }
    if app.is_file_editor_open() {
        app.file_editor_key(key);
        return;
    }
    if app.is_cast_editor_open() {
        app.cast_editor_key(key);
        return;
    }
    if app.is_preview_open() {
        handle_preview_key(app, key);
        return;
    }
    if app.is_help_open() {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1) => app.close_help(),
            _ => {}
        }
        return;
    }
    if app.is_palette_open() {
        handle_palette_key(app, key);
        return;
    }
    if app.is_comment_prompt_open() {
        app.comment_prompt_key(key);
        return;
    }
    if app.is_cast_confirmation_open() {
        app.cast_confirmation_key(key);
        return;
    }
    if app.is_deploy_picker_open() {
        app.deploy_picker_key(key);
        return;
    }
    if app.is_launch_picker_open() {
        app.launch_picker_key(key);
        return;
    }
    if route_text_input(app, key) {
        return;
    }
    if app.navigation_prefix_key(key) {
        return;
    }
    if app.is_visual_mode() {
        app.visual_key(key);
        return;
    }
    // Digits always address the numbered detail tabs; sections are reached
    // by navigation, the palette, and the letter shortcuts (t/h/c/m) shown
    // in the Sections column.
    if switch_detail_tab_for_digit(app, key) {
        return;
    }
    if switch_section_for_shortcut(app, key) {
        return;
    }

    if !matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
        app.disarm_quit();
    }
    match key.code {
        KeyCode::Esc => app.escape(),
        KeyCode::Char('q') => app.request_quit(),
        KeyCode::Char('?') | KeyCode::F(1) => app.toggle_help(),
        KeyCode::Char(':') => app.open_palette(),
        KeyCode::Char('/') => app.begin_list_filter(),
        KeyCode::Char('!') => app.toggle_problems_only(),
        KeyCode::Char('r') => app.refresh(),
        KeyCode::Char('e') => app.edit_selected_source_or_cast(),
        KeyCode::Char('E') => app.open_selected_source_external(),
        KeyCode::Char('y' | 'Y') => app.copy_tuicr_review(),
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => app.drill_or_expand(),
        KeyCode::Left | KeyCode::Char('h') => app.move_back(),
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => app.focus_previous(),
        KeyCode::BackTab => app.focus_previous(),
        KeyCode::Tab => app.focus_next(),
        KeyCode::Char('p') => app.preview_or_previous_section(),
        KeyCode::Char('c') => app.comment_or_code(),
        KeyCode::Char('d') if app.is_comment_navigator_focused() => {
            app.delete_selected_comment();
        }
        KeyCode::Char('d') => app.set_detail_tab(super::app::DetailTab::Diff),
        KeyCode::Char('v') => app.set_detail_tab(super::app::DetailTab::Provenance),
        KeyCode::Char('f') => app.set_detail_tab(super::app::DetailTab::Frontmatter),
        KeyCode::Char('i') => app.set_detail_tab(super::app::DetailTab::History),
        KeyCode::Char('o') => app.open_user_override_or_repo(),
        KeyCode::Char('O') => app.open_repo_tool(true),
        KeyCode::Char('D') => app.open_deploy_picker(),
        KeyCode::Char('L') => app.launch_harness(),
        KeyCode::Char('H') => app.open_history_for_selection(),
        _ => app.focused_key(key),
    }
}

fn switch_detail_tab_for_digit(app: &mut App, key: KeyEvent) -> bool {
    let KeyCode::Char(digit @ '1'..='6') = key.code else {
        return false;
    };
    app.set_detail_tab(super::app::DetailTab::ALL[usize::from(digit as u8 - b'1')]);
    true
}

fn route_text_input(app: &mut App, key: KeyEvent) -> bool {
    if !matches!(
        key.code,
        KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Esc | KeyCode::Enter
    ) {
        return false;
    }
    if app.is_code_search_input_active() {
        app.code_search_input_key(key);
    } else if app.is_search_input_active() {
        app.search_input_key(key);
    } else if app.is_list_filter_typing() {
        app.list_filter_key(key);
    } else {
        return false;
    }
    true
}

fn switch_section_for_shortcut(app: &mut App, key: KeyEvent) -> bool {
    app.has_section_digit_shortcuts()
        && matches!(key.code, KeyCode::Char(character) if app.set_section_by_shortcut(character))
}

fn handle_preview_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => app.close_preview(),
        KeyCode::Down | KeyCode::Char('j') => app.preview_scroll_down(1),
        KeyCode::Up | KeyCode::Char('k') => app.preview_scroll_up(1),
        KeyCode::PageDown | KeyCode::Char(' ') => app.preview_scroll_down(10),
        KeyCode::PageUp | KeyCode::Char('b') => app.preview_scroll_up(10),
        KeyCode::Home | KeyCode::Char('g') => app.preview_scroll_to_top(),
        KeyCode::End | KeyCode::Char('G') => app.preview_scroll_to_bottom(),
        KeyCode::Char(digit @ '1'..='6') => {
            let index = usize::from(digit as u8 - b'1');
            app.set_detail_tab(super::app::DetailTab::ALL[index]);
        }
        KeyCode::Char('p') => app.set_detail_tab(super::app::DetailTab::Preview),
        KeyCode::Char('c') => app.set_detail_tab(super::app::DetailTab::Code),
        KeyCode::Char('d') => app.set_detail_tab(super::app::DetailTab::Diff),
        KeyCode::Char('v') => app.set_detail_tab(super::app::DetailTab::Provenance),
        KeyCode::Char('f') => app.set_detail_tab(super::app::DetailTab::Frontmatter),
        KeyCode::Char('i') => app.set_detail_tab(super::app::DetailTab::History),
        _ => {}
    }
}

fn handle_palette_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.close_palette(),
        KeyCode::Enter => app.execute_palette(),
        _ => app.palette_key(key),
    }
}
