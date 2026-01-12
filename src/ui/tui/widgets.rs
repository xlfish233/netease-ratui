use ratatui::widgets::ListState;

pub(super) fn list_state(selected: usize) -> ListState {
    let mut st = ListState::default();
    st.select(Some(selected));
    st
}

pub(super) fn progress_bar_text(elapsed_ms: u64, total_ms: Option<u64>, width: usize) -> String {
    let Some(total_ms) = total_ms.filter(|t| *t > 0) else {
        return "进度: [------------------------]".to_owned();
    };

    let ratio = (elapsed_ms.min(total_ms) as f64) / (total_ms as f64);
    let filled = ((ratio * width as f64).round() as usize).min(width);
    let bar = "#".repeat(filled) + &"-".repeat(width - filled);
    format!("进度: [{bar}]")
}
