use ratatui::layout::{Constraint, Direction, Layout, Rect};

const HEADER_HEIGHT: u16 = 3;
const FOOTER_HEIGHT: u16 = 3;
const TOAST_HEIGHT: u16 = 3;

pub(super) struct CanvasLayout {
    pub header: Rect,
    pub body: Rect,
    pub toast: Rect,
    pub footer: Rect,
}

pub(super) struct HeaderLayout {
    pub tabs: Rect,
    pub search: Rect,
    pub status: Rect,
}

pub(super) struct BodyLayout {
    pub left: Rect,
    pub center: Rect,
    pub right: Rect,
}

pub(super) struct RightLayout {
    pub context: Rect,
    pub now: Rect,
}

pub(super) fn split_canvas(canvas: Rect) -> CanvasLayout {
    let body_height = canvas
        .height
        .saturating_sub(HEADER_HEIGHT + FOOTER_HEIGHT + TOAST_HEIGHT);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(HEADER_HEIGHT),
            Constraint::Length(body_height),
            Constraint::Length(TOAST_HEIGHT),
            Constraint::Length(FOOTER_HEIGHT),
        ])
        .split(canvas);

    CanvasLayout {
        header: chunks[0],
        body: chunks[1],
        toast: chunks[2],
        footer: chunks[3],
    }
}

pub(super) fn split_header(header: Rect) -> HeaderLayout {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(header);

    HeaderLayout {
        tabs: rows[0],
        search: rows[1],
        status: rows[2],
    }
}

pub(super) fn split_body(body: Rect) -> BodyLayout {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Length(58),
            Constraint::Length(40),
        ])
        .split(body);

    BodyLayout {
        left: cols[0],
        center: cols[1],
        right: cols[2],
    }
}

pub(super) fn split_right(right: Rect) -> RightLayout {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(right);

    RightLayout {
        context: rows[0],
        now: rows[1],
    }
}
