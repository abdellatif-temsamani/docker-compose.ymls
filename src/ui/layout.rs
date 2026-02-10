use ratatui::layout::{Constraint, Layout, Rect};

pub struct Sections {
    pub status_bar: Rect,
    pub services_list: Rect,
    pub logs: Rect,
    pub search: Option<Rect>,
    pub help: Rect,
}

pub fn build(area: Rect, show_search: bool) -> Sections {
    let outer = if area.width > 80 && area.height > 20 {
        Rect {
            x: area.x.saturating_add(1),
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        }
    } else {
        area
    };

    let controls_height = controls_height(area.height);
    let [status_bar, content, controls] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(controls_height),
    ])
    .areas(outer);

    let services_percentage = services_width_percentage(area.width);
    let [services, logs] = Layout::horizontal([
        Constraint::Percentage(services_percentage),
        Constraint::Percentage(100 - services_percentage),
    ])
    .areas(content);

    let (search, services_list) = if show_search {
        let [search, services_list] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(services);
        (Some(search), services_list)
    } else {
        (None, services)
    };

    Sections {
        status_bar,
        services_list,
        logs,
        search,
        help: controls,
    }
}

fn controls_height(frame_height: u16) -> u16 {
    if frame_height < 15 {
        2
    } else if frame_height < 20 {
        2
    } else if frame_height < 30 {
        3
    } else {
        4
    }
}

fn services_width_percentage(frame_width: u16) -> u16 {
    if frame_width < 80 {
        25
    } else if frame_width < 120 {
        30
    } else {
        35
    }
}
