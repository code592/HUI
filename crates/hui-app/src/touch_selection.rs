//! Small adapter to Slint 1.17's native text layout. Keep the dependency pinned:
//! hit-testing and handle placement must use the same renderer/IME input as painting.
use crate::{AppWindow, TouchSelection};
use i_slint_core::{
    accessibility::AccessibleStringProperty, item_tree::ItemRc, items::TextInput,
    lengths::LogicalPoint, window::WindowInner,
};
use slint::ComponentHandle;
use std::ops::ControlFlow;

fn input_item(ui: &AppWindow) -> Option<ItemRc> {
    let label = if ui.get_mode() == 2 {
        "HUI inline source"
    } else {
        "HUI source"
    };
    let root = ItemRc::new_root(WindowInner::from_pub(ui.window()).component());
    root.visit_descendants(|item| {
        if item.downcast::<TextInput>().is_some()
            && item
                .accessible_string_property(AccessibleStringProperty::Label)
                .is_some_and(|s| s == label)
        {
            ControlFlow::Break(item.clone())
        } else {
            ControlFlow::Continue(())
        }
    })
}

pub fn gesture(ui: &AppWindow, phase: i32, x: f32, y: f32) {
    ui.set_source_all_selected(false);
    let Some(item) = input_item(ui) else {
        return;
    };
    let Some(input) = item.downcast::<TextInput>() else {
        return;
    };
    let input = input.as_pin_ref();
    let adapter = WindowInner::from_pub(ui.window()).window_adapter();
    let text = ui.get_source();
    let byte = input.byte_offset_for_position(LogicalPoint::new(x, y), &adapter, &item);
    let (a, b) = match phase {
        0 => (byte, byte),
        3 => {
            let word = hui_core::text_selection::word_at(&text, byte);
            (word.start, word.end)
        }
        4 => (byte, ui.get_source_cursor().max(0) as usize),
        5 => (ui.get_source_anchor().max(0) as usize, byte),
        6 => (0, text.len()),
        _ => {
            ui.invoke_select_source(ui.get_source_cursor(), ui.get_source_cursor());
            ui.set_edit_selection(TouchSelection::default());
            return;
        }
    };
    ui.invoke_select_source(a as i32, b as i32);
    #[cfg(target_os = "android")]
    if phase == 0 {
        // A touch overlay bypasses TextInput's ensure_focus_and_ime. Re-enable the
        // real native IME on a tap even when Back hid it without changing focus.
        use i_slint_core::{
            input::{FocusEvent, FocusReason},
            items::Item,
        };
        input.focus_event(
            &FocusEvent::FocusIn(FocusReason::PointerClick),
            &adapter,
            &item,
        );
    }
    ui.set_all_selected_source(ui.get_source());
    ui.set_source_all_selected(phase == 6 && ui.get_mode() != 2);
    refresh(ui);
}

pub fn refresh(ui: &AppWindow) {
    if !ui.get_mobile() {
        return;
    }
    let a = ui.get_source_anchor().max(0) as usize;
    let b = ui.get_source_cursor().max(0) as usize;
    // Native caret navigation cancels whole-document selection. An actual text
    // edit changes the buffer first; retain the flag until the controller consumes it.
    if ui.get_source_all_selected()
        && ui.get_source() == ui.get_all_selected_source()
        && !(a.min(b) == 0 && a.max(b) == ui.get_source().len())
    {
        ui.set_source_all_selected(false);
    }
    if a == b {
        ui.set_edit_selection(TouchSelection::default());
        return;
    }
    let Some(item) = input_item(ui) else {
        return;
    };
    let Some(input) = item.downcast::<TextInput>() else {
        return;
    };
    let adapter = WindowInner::from_pub(ui.window()).window_adapter();
    let rect = |offset| {
        adapter
            .renderer()
            .text_input_cursor_rect_for_byte_offset(input.as_pin_ref(), &item, offset)
    };
    let a = rect(a);
    let b = rect(b);
    ui.set_edit_selection(TouchSelection {
        active: true,
        ax: a.min_x(),
        ay: a.max_y(),
        bx: b.min_x(),
        by: b.max_y(),
    });
}

#[cfg(target_os = "android")]
pub fn probe_geometry(ui: &AppWindow, byte: usize) -> serde_json::Value {
    let window = WindowInner::from_pub(ui.window());
    let root = ItemRc::new_root(window.component());
    let item = if ui.get_mode() == 3 {
        root.visit_descendants(|item| {
            if item
                .accessible_string_property(AccessibleStringProperty::Label)
                .is_some_and(|s| s == "HUI reading surface")
            {
                ControlFlow::Break(item.clone())
            } else {
                ControlFlow::Continue(())
            }
        })
    } else {
        input_item(ui)
    };
    let Some(item) = item else {
        return serde_json::json!({"error":"missing text surface"});
    };
    let origin = item.map_to_window(LogicalPoint::new(0., 0.));
    let selection = if ui.get_mode() == 3 {
        ui.get_reading_selection()
    } else {
        ui.get_edit_selection()
    };
    let caret = item.downcast::<TextInput>().map(|input| {
        window
            .window_adapter()
            .renderer()
            .text_input_cursor_rect_for_byte_offset(
                input.as_pin_ref(),
                &item,
                byte.min(ui.get_source().len()),
            )
    });
    let mut hits = Vec::new();
    let point = LogicalPoint::new(origin.x + selection.bx, origin.y + selection.by + 14.);
    root.visit_descendants::<()>(|candidate| {
        if candidate.is_visible() && let Some(area) = candidate.downcast::<i_slint_core::items::TouchArea>() {
            let pos = candidate.map_to_window(LogicalPoint::new(0.,0.));
            let size = candidate.geometry().size;
            if i_slint_core::lengths::LogicalRect::new(pos,size).contains(point) || area.as_pin_ref().pressed() {
                hits.push(serde_json::json!({"x":pos.x,"y":pos.y,"w":size.width,"h":size.height,
                    "pressed":area.as_pin_ref().pressed(),"mx":area.as_pin_ref().mouse_x().get(),"my":area.as_pin_ref().mouse_y().get()}));
            }
        }
        ControlFlow::Continue(())
    });
    serde_json::json!({"hits":hits,"active":selection.active,"ax":origin.x+selection.ax,"ay":origin.y+selection.ay,
        "bx":origin.x+selection.bx,"by":origin.y+selection.by,
        "x":origin.x+caret.map(|r|r.min_x()+2.).unwrap_or(32.),
        "y":origin.y+caret.map(|r|r.center().y).unwrap_or(32.),
        "anchor":ui.get_source_anchor(),"cursor":ui.get_source_cursor(),
        "all":ui.get_source_all_selected(),"source":ui.get_source().as_str(),"busy":ui.get_busy()})
}
