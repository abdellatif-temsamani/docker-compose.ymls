use std::io;

use ratatui::Frame;

use crate::app::{App, Focus};

mod controls;
mod layout;
mod logs;
mod overlays;
mod services;
mod status_bar;

pub fn render_ui(frame: &mut Frame, app: &mut App) -> io::Result<()> {
    let show_search = app.focus == Focus::Services && app.search_mode;
    let sections = layout::build(frame.area(), show_search);

    status_bar::render(frame, app, sections.status_bar);
    services::render(frame, app, sections.services_list, sections.search);
    logs::render(frame, app, sections.logs);
    controls::render(frame, app, sections.help);
    overlays::render(frame, app);

    Ok(())
}
