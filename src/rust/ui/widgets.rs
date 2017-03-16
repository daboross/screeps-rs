use conrod::{self, color};

pub fn main_screen(ui: &mut conrod::UiCell, ids: &Ids) {
    use conrod::{Colorable, Labelable, Positionable, Sizeable, Widget};
    use conrod::widget::{Canvas, DropDownList};

    let header = Canvas::new().color(color::BLUE).pad_bottom(20.0);

    let body = Canvas::new();

    Canvas::new()
        .flow_down(&[(ids.header, header), (ids.body, body)])
        .set(ids.master, ui);

    let items = ["Item A.", "Item B.", "Item C."];
    let list = DropDownList::new(&items, None)
        .scrollbar_next_to()
        .left_justify_label()
        .wh([200.0, 100.0])
        .top_left_of(ids.body)
        .label("Items.")
        .small_font(&ui);

    let selected_option = list.set(ids.list, ui);

    if let Some(v) = selected_option {
        println!("Selected option {}.", v);
    }
}

widget_ids! {
    pub struct Ids {
        master,
        header,
        body,
        list,
    }
}
