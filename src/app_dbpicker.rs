//! The database switcher in the title bar (child module of `app`): a search
//! box over the server's databases. Same shape as the connection switcher; the
//! kit's menu has no search, and a server can hold hundreds of databases.

use gpui_kit::component::input::InputEvent;

use super::*;

const WIDTH: f32 = 280.;
/// Rows shown before the list scrolls.
const SHOWN: usize = 12;

pub struct DbPicker {
    names: Vec<String>,
    active: String,
    query: Entity<InputState>,
    /// Indexes into `names` that match the query.
    matches: Vec<usize>,
    pick: usize,
    scroll: UniformListScrollHandle,
    _sub: Subscription,
}

impl DbPicker {
    pub fn new(
        names: Vec<String>,
        active: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search databases…"));
        let _sub = cx.subscribe_in(
            &query,
            window,
            |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => this.refilter(cx),
                InputEvent::PressEnter { .. } => this.confirm(None, window, cx),
                _ => {}
            },
        );
        let pick = names.iter().position(|n| *n == active).unwrap_or(0);
        let scroll = UniformListScrollHandle::new();
        scroll.scroll_to_item(pick, ScrollStrategy::Center);
        Self {
            matches: (0..names.len()).collect(),
            names,
            active,
            query,
            pick,
            scroll,
            _sub,
        }
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.query.read(cx).focus_handle(cx)
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let q = self.query.read(cx).value().to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        self.matches = (0..self.names.len())
            .filter(|&i| {
                let hay = self.names[i].to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect();
        self.pick = 0;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.matches.len();
        if n == 0 {
            return;
        }
        self.pick = (self.pick as isize + delta).rem_euclid(n as isize) as usize;
        self.scroll
            .scroll_to_item(self.pick, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Switch to the highlighted database, or to the one at `row`.
    fn confirm(&mut self, row: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&m) = self.matches.get(row.unwrap_or(self.pick)) else {
            return;
        };
        let name = self.names[m].clone();
        let view = cx.global::<TuskHandle>().0.clone();
        view.update(cx, |app, cx| {
            app.db_picker = None;
            app.switch_database(name, window, cx);
            cx.notify();
        });
    }
}

impl Render for DbPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let (fg, muted) = (t.foreground, t.muted_foreground);
        let row_h = crate::settings::row_h();
        let n = self.matches.len();
        let this = cx.entity();
        let list = uniform_list("db-picker-list", n, move |range, _, cx| {
            let p = this.read(cx);
            range
                .map(|r| {
                    let name = &p.names[p.matches[r]];
                    let selected = *name == p.active;
                    let this = this.clone();
                    div()
                        .id(("db-picker-row", r))
                        .w_full()
                        .h(px(row_h))
                        .px_2()
                        .mx_1()
                        .rounded(crate::theme::RADIUS_SM)
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(crate::settings::ui_text()))
                        .when(r == p.pick, |d| d.bg(t.tokens.table_active))
                        .hover(|d| d.bg(muted.opacity(0.12)))
                        .child(div().w(px(12.)).flex_none().when(selected, |d| {
                            d.child(Icon::new(IconName::Check).size(px(12.)).text_color(fg))
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(fg)
                                .child(name.clone()),
                        )
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            this.update(cx, |p, cx| p.confirm(Some(r), window, cx));
                        })
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.scroll)
        .h(px(n.clamp(1, SHOWN) as f32 * row_h));

        div()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .gap_1()
            .key_context("DbPicker")
            .on_action(cx.listener(|this, _: &DbPickUp, _, cx| this.step(-1, cx)))
            .on_action(cx.listener(|this, _: &DbPickDown, _, cx| this.step(1, cx)))
            .child(Input::new(&self.query).small())
            .child(if n == 0 {
                div()
                    .p_2()
                    .text_caption()
                    .text_color(muted)
                    .child("No match")
                    .into_any_element()
            } else {
                list.into_any_element()
            })
    }
}

impl TuskApp {
    /// Open the database switcher and focus its search box.
    pub(super) fn open_db_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let names = self.databases.clone();
        let active = self.current_database();
        let picker = cx.new(|cx| DbPicker::new(names, active, window, cx));
        let focus = picker.read(cx).focus_handle(cx);
        window.defer(cx, move |window, cx| focus.focus(window, cx));
        self.db_picker = Some(picker);
        cx.notify();
    }
}
