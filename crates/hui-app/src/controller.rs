use crate::i18n::{self, message, tr};
use crate::{AppWindow, FileRow, TabRow, worker};
use hui_core::{Document, FileStamp, LocalStorage, Selection, Viewport, storage::Preferences};
use ropey::Rope;
use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const WINDOW_CHARS: usize = 8192;
pub(crate) type State = Rc<RefCell<App>>;
type RenderKey = worker::RenderCacheKey;

struct Tab {
    id: u64,
    name: String,
    path: Option<PathBuf>,
    stamp: Option<FileStamp>,
    document: Document,
    view: Viewport,
    pending_save: Option<(PathBuf, Rope)>,
    recovery: PathBuf,
    recovery_revision: u64,
}
pub struct App {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    files: Vec<PathBuf>,
    showing_recent: bool,
    prefs: Preferences,
    data: PathBuf,
    render: worker::Renderer,
    render_key: Option<RenderKey>,
    clipboard: Option<arboard::Clipboard>,
    debounce: Timer,
    autosave: Timer,
    search_cancel: Arc<AtomicBool>,
}
impl App {
    fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }
    fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }
    fn add(&mut self, text: &str, path: Option<PathBuf>, stamp: Option<FileStamp>, name: String) {
        self.add_rope(Rope::from_str(text), path, stamp, name);
    }
    fn add_rope(
        &mut self,
        rope: Rope,
        path: Option<PathBuf>,
        stamp: Option<FileStamp>,
        name: String,
    ) {
        let id = self.next_id;
        self.next_id += 1;
        let document = Document::from_rope(rope);
        let pos = path
            .as_ref()
            .and_then(|p| self.prefs.positions.get(p))
            .copied()
            .unwrap_or(0);
        let view = document.viewport(pos, WINDOW_CHARS);
        self.tabs.push(Tab {
            id,
            document,
            view,
            name,
            path,
            stamp,
            pending_save: None,
            recovery_revision: 0,
            recovery: self.data.join(format!("recovery-{id}.md")),
        });
        self.active = self.tabs.len() - 1;
    }
    fn open(&mut self, path: PathBuf, ui: &AppWindow) {
        let path = path.canonicalize().unwrap_or(path);
        if let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.path.as_ref() == Some(&path))
        {
            self.active = index;
            self.refresh(ui, true);
            return;
        }
        match LocalStorage::read(&path) {
            Ok((rope, stamp)) => {
                self.prefs.recent.retain(|p| p != &path);
                self.prefs.recent.insert(0, path.clone());
                self.prefs.recent.truncate(40);
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.add_rope(rope, Some(path), Some(stamp), name);
                self.refresh(ui, true);
                self.persist();
            }
            Err(err) => ui.set_notice(tr(&err.to_string()).into()),
        }
    }
    pub fn persist(&mut self) {
        for tab in &self.tabs {
            if let Some(p) = &tab.path {
                self.prefs.positions.insert(p.clone(), tab.view.chars.start);
            }
        }
        if let Err(e) = self.prefs.store(&self.data.join("preferences.json")) {
            eprintln!("preferences: {e}");
        }
    }
    fn refresh(&mut self, ui: &AppWindow, replace_text: bool) {
        let tab = self.tab();
        ui.set_document_name(tab.name.clone().into());
        ui.set_location(
            tab.path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| tr("本地工作空间"))
                .into(),
        );
        ui.set_active_tab(self.active as i32);
        ui.set_tabs(ModelRc::new(VecModel::from(
            self.tabs
                .iter()
                .map(|t| TabRow {
                    label: t.name.clone().into(),
                    dirty: t.document.is_dirty(),
                })
                .collect::<Vec<_>>(),
        )));
        ui.set_dirty(tab.document.is_dirty());
        ui.set_status(
            if tab.pending_save.is_some() {
                tr("正在保存…")
            } else if tab.document.is_dirty() {
                tr("有未保存的修改")
            } else if tab.path.is_none() {
                tr("本地工作空间")
            } else {
                tr("已保存到本地")
            }
            .into(),
        );
        ui.set_statistics(
            message(
                "{chars} 字符  ·  {lines} 行  ·  UTF-8",
                &[
                    ("chars", tab.document.len_chars().to_string()),
                    ("lines", tab.document.len_lines().to_string()),
                ],
            )
            .into(),
        );
        ui.set_source_label(
            message(
                "MARKDOWN    ·    第 {line} 行起",
                &[("line", (tab.view.first_line + 1).to_string())],
            )
            .into(),
        );
        ui.set_has_previous(tab.view.chars.start > 0);
        ui.set_has_next(tab.view.chars.end < tab.document.len_chars());
        ui.set_document_position(
            tab.view.chars.start as f32 / tab.document.len_chars().max(1) as f32,
        );
        if replace_text {
            ui.set_source(tab.view.text.clone().into());
        }
    }
    fn navigate(&mut self, position: usize, ui: &AppWindow) {
        let tab = self.tab_mut();
        tab.view = tab.document.viewport(position, WINDOW_CHARS);
        self.refresh(ui, true);
    }
    fn sync_selection(&mut self, ui: &AppWindow) {
        let t = self.tab_mut();
        let chars = |byte: i32| {
            t.view
                .text
                .char_indices()
                .take_while(|(i, _)| *i < byte.max(0) as usize)
                .count()
                + t.view.chars.start
        };
        t.document.selection = Selection {
            anchor: chars(ui.get_source_anchor()),
            head: chars(ui.get_source_cursor()),
        };
    }
    fn current_render_key(&self, ui: &AppWindow) -> RenderKey {
        let t = self.tab();
        (
            t.id,
            t.document.revision(),
            ui.get_preview_width() as u32,
            ui.get_preview_height() as u32,
            ui.window().scale_factor().to_bits(),
            ui.get_theme(),
            ui.get_font_size().to_bits(),
            ui.get_reading_width().to_bits(),
            ui.get_line_height().to_bits(),
            ui.get_paragraph_spacing().to_bits(),
            i18n::current(),
        )
    }
    fn request_render(&mut self, ui: &AppWindow) {
        self.request_render_inner(ui, false);
    }
    fn request_formula_preview(&self, ui: &AppWindow) {
        self.render.formula_preview(
            ui.get_source().to_string(),
            (ui.get_inline_width() * 0.5).max(160.),
            ui.get_inline_height().max(180.),
            ui.window().scale_factor(),
            ui.get_theme(),
            ui.get_font_size(),
        );
    }
    fn request_render_inner(&mut self, ui: &AppWindow, while_inline: bool) {
        if !while_inline && ui.get_mode() == 2 && ui.get_editor_active() {
            if ui.get_inline_math() {
                self.request_formula_preview(ui);
            }
            return;
        }
        if ui.get_mode() == 0 {
            ui.set_displayed_mode(0);
            if ui.get_busy() {
                self.render.cancel();
                self.render_key = None;
            }
            ui.set_busy(false);
            return;
        }
        let key = self.current_render_key(ui);
        if self.render_key.as_ref() == Some(&key) {
            if !ui.get_busy() {
                ui.set_displayed_mode(ui.get_mode());
            }
            return;
        }
        let t = self.tab();
        let text = t.document.snapshot();
        let root = t
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        self.render_key = Some(key);
        self.render.document(
            key,
            text,
            root,
            ui.get_preview_width().max(100.) as u32,
            ui.get_preview_height().max(100.) as u32,
            ui.window().scale_factor(),
            ui.get_theme(),
            ui.get_font_size(),
            ui.get_reading_width(),
            ui.get_line_height(),
            ui.get_paragraph_spacing(),
            ui.get_preview_position(),
        );
        ui.set_busy(true);
    }
    fn show_files(&self, ui: &AppWindow) {
        ui.set_files(ModelRc::new(VecModel::from(
            self.files
                .iter()
                .map(|p| FileRow {
                    label: p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                        .into(),
                    detail: p.display().to_string().into(),
                    folder: p.is_dir(),
                    depth: 0,
                })
                .collect::<Vec<_>>(),
        )));
    }
    fn folder(&mut self, path: &Path, ui: &AppWindow) {
        match std::fs::read_dir(path) {
            Ok(entries) => {
                self.showing_recent = false;
                self.files = entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.is_dir() || is_markdown(p))
                    .collect();
                self.files.sort_by_key(|p| (!p.is_dir(), p.clone()));
                ui.set_folder_name(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                        .into(),
                );
                self.show_files(ui);
            }
            Err(e) => ui.set_notice(tr(&e.to_string()).into()),
        }
    }
}
fn is_markdown(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}
pub fn schedule_render(app: &State, ui: &AppWindow) {
    let weak = ui.as_weak();
    let state = Rc::downgrade(app);
    app.borrow().debounce.start(
        TimerMode::SingleShot,
        Duration::from_millis(180),
        move || {
            if let (Some(ui), Some(app)) = (weak.upgrade(), state.upgrade()) {
                app.borrow_mut().request_render(&ui);
            }
        },
    );
}
fn schedule_save(app: &State, ui: &AppWindow) {
    let weak = ui.as_weak();
    let state = Rc::downgrade(app);
    app.borrow()
        .autosave
        .start(TimerMode::SingleShot, Duration::from_secs(1), move || {
            if let (Some(ui), Some(app)) = (weak.upgrade(), state.upgrade()) {
                let count = app.borrow().tabs.len();
                for index in 0..count {
                    save_index(&app, &ui, false, true, index);
                }
            }
        });
}
fn save(app: &State, ui: &AppWindow, save_as: bool, automatic: bool) {
    let index = app.borrow().active;
    save_index(app, ui, save_as, automatic, index);
}
fn save_index(app: &State, ui: &AppWindow, save_as: bool, automatic: bool, index: usize) {
    let mut a = app.borrow_mut();
    let tab = &mut a.tabs[index];
    if tab.pending_save.is_some() || (automatic && !tab.document.is_dirty()) {
        return;
    }
    let path = if save_as || (tab.path.is_none() && !automatic) {
        rfd::FileDialog::new()
            .set_title(tr("保存 Markdown"))
            .add_filter("Markdown", &["md", "markdown"])
            .set_file_name(&tab.name)
            .save_file()
    } else {
        tab.path.clone().or_else(|| {
            (tab.document.revision() != tab.recovery_revision).then(|| tab.recovery.clone())
        })
    };
    let snapshot = tab.document.snapshot();
    let recovery = tab.recovery.clone();
    let expected = if path == tab.path {
        tab.stamp.clone()
    } else {
        None
    };
    let id = tab.id.to_string();
    let weak = ui.as_weak();
    if let Some(path) = path {
        tab.pending_save = Some((path.clone(), snapshot.clone()));
        ui.set_status(tr("正在保存…").into());
        std::thread::spawn(move || {
            let result = LocalStorage::save(&path, &snapshot, expected.as_ref());
            let (stamp, error) = match result {
                Ok(stamp) => (stamp.0.to_hex().to_string(), String::new()),
                Err(e) => {
                    let _ = LocalStorage::save(&recovery, &snapshot, None);
                    (String::new(), e.to_string())
                }
            };
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.invoke_save_finished(id.into(), stamp.into(), error.into())
            });
        });
    }
}
fn image_markdown(a: &App, bytes: &[u8], extension: &str) -> anyhow::Result<String> {
    use std::io::Write;
    anyhow::ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        tr("图片大小不得超过 32 MB")
    );
    let root = a
        .tab()
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .ok_or_else(|| {
            anyhow::anyhow!(tr(
                "请先保存 Markdown 文档，再插入图片；图片将保存到同目录 assets 文件夹"
            ))
        })?;
    let assets = root.join("assets");
    std::fs::create_dir_all(&assets)?;
    anyhow::ensure!(
        assets.canonicalize()?.starts_with(root.canonicalize()?),
        tr("assets 文件夹不能指向文档目录之外")
    );
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let name = format!("image-{stamp}.{extension}");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(assets.join(&name))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(format!("![{}](assets/{name})", tr("图片")))
}
fn insert_source(a: &mut App, ui: &AppWindow, insert: String) -> anyhow::Result<()> {
    a.sync_selection(ui);
    let t = a.tab_mut();
    let selection = &t.document.selection;
    let range = selection.anchor.min(selection.head)..selection.anchor.max(selection.head);
    let old_len = t.document.len_chars();
    t.document.apply(
        t.document.revision(),
        vec![hui_core::Edit { range, insert }],
    )?;
    let count = (t.view.chars.len() as isize + t.document.len_chars() as isize - old_len as isize)
        .max(0) as usize;
    t.view = t.document.viewport(t.view.chars.start, count);
    let byte = t
        .view
        .text
        .chars()
        .take(t.document.selection.head.saturating_sub(t.view.chars.start))
        .map(char::len_utf8)
        .sum::<usize>() as i32;
    a.refresh(ui, true);
    ui.invoke_select_source(byte, byte);
    Ok(())
}
fn source_clipboard(app: &State, ui: &AppWindow, command: &str) {
    if ui.get_mode() == 3 || (ui.get_mode() == 2 && !ui.get_editor_active()) {
        return;
    }
    let mut a = app.borrow_mut();
    a.sync_selection(ui);
    let result = (|| -> anyhow::Result<()> {
        if a.clipboard.is_none() {
            a.clipboard = Some(arboard::Clipboard::new()?);
        }
        let s = &a.tab().document.selection;
        let range = s.anchor.min(s.head)..s.anchor.max(s.head);
        if command != "paste-source" {
            let text = a.tab().document.snapshot().slice(range.clone()).to_string();
            if text.is_empty() {
                return Ok(());
            }
            a.clipboard.as_mut().unwrap().set_text(text)?;
            if command == "copy-source" {
                return Ok(());
            }
        }
        let insert = if command == "paste-source" {
            match a.clipboard.as_mut().unwrap().get_text() {
                Ok(text) => text,
                Err(_) => {
                    let image = a.clipboard.as_mut().unwrap().get_image()?;
                    anyhow::ensure!(image.bytes.len() <= 128 * 1024 * 1024, tr("剪贴板图片过大"));
                    let bytes = hui_render::native::encode_png(
                        image.width as u32,
                        image.height as u32,
                        &image.bytes,
                    )?;
                    image_markdown(&a, &bytes, "png")?
                }
            }
        } else {
            String::new()
        };
        let t = a.tab_mut();
        let old_len = t.document.len_chars();
        t.document.apply(
            t.document.revision(),
            vec![hui_core::Edit { range, insert }],
        )?;
        let count = (t.view.chars.len() as isize + t.document.len_chars() as isize
            - old_len as isize)
            .max(0) as usize;
        t.view = t.document.viewport(t.view.chars.start, count);
        let byte = t
            .view
            .text
            .chars()
            .take(t.document.selection.head.saturating_sub(t.view.chars.start))
            .map(char::len_utf8)
            .sum::<usize>() as i32;
        a.refresh(ui, true);
        ui.invoke_select_source(byte, byte);
        Ok(())
    })();
    if let Err(e) = result {
        ui.set_notice(message("剪贴板操作失败：{error}", &[("error", tr(&e.to_string()))]).into());
    }
    drop(a);
    if command != "copy-source" {
        schedule_save(app, ui);
        schedule_render(app, ui);
    }
}
fn action(app: &State, ui: &AppWindow, name: &str) {
    match name {
        "copy-source" | "cut-source" | "paste-source" => {
            source_clipboard(app, ui, name);
            return;
        }
        "insert-image" => {
            if ui.get_mode() == 3 || (ui.get_mode() == 2 && !ui.get_editor_active()) {
                ui.set_notice(tr("请先在源码或即时模式中放置光标").into());
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(tr("图片"), &["png", "jpg", "jpeg", "gif", "webp", "svg"])
                .pick_file()
            {
                let result = (|| -> anyhow::Result<()> {
                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("png")
                        .to_ascii_lowercase();
                    anyhow::ensure!(
                        std::fs::metadata(&path)?.len() <= 32 * 1024 * 1024,
                        tr("图片大小不得超过 32 MB")
                    );
                    let mut a = app.borrow_mut();
                    let markdown = image_markdown(&a, &std::fs::read(&path)?, &ext)?;
                    insert_source(&mut a, ui, markdown)
                })();
                if let Err(e) = result {
                    ui.set_notice(
                        message("插入图片失败：{error}", &[("error", tr(&e.to_string()))]).into(),
                    );
                }
                schedule_save(app, ui);
            }
        }
        "mode-change" => {
            let mut a = app.borrow_mut();
            if ui.get_editor_active() {
                a.render_key = None;
                ui.set_editor_active(false);
            }
            if ui.get_mode() == 0 {
                let pos = a.tab().view.chars.start;
                a.navigate(pos, ui);
            }
            a.request_render(ui);
            return;
        }
        "finish-inline" => {
            app.borrow_mut().render_key = None;
        }
        "open" => {
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("Markdown", &["md", "markdown", "txt"])
                .pick_files()
            {
                let mut a = app.borrow_mut();
                for p in paths {
                    a.open(p, ui);
                }
                if a.showing_recent {
                    a.files = a.prefs.recent.clone();
                    a.show_files(ui);
                }
            }
        }
        "folder" => {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                app.borrow_mut().folder(&path, ui);
            }
        }
        "new" => {
            let mut a = app.borrow_mut();
            a.add("", None, None, tr("未命名.md"));
            a.refresh(ui, true);
            ui.set_mode(0);
            ui.invoke_focus_editor();
        }
        "close-all" => {
            close_all_tabs(app, ui);
        }
        "save" => save(app, ui, false, false),
        "save-as" => save(app, ui, true, false),
        "undo" | "redo" => {
            let mut a = app.borrow_mut();
            let t = a.tab_mut();
            let old_len = t.document.len_chars();
            if name == "undo" {
                t.document.undo();
            } else {
                t.document.redo();
            }
            if ui.get_mode() == 2 && ui.get_editor_active() {
                let count = (t.view.chars.len() as isize + t.document.len_chars() as isize
                    - old_len as isize)
                    .max(0) as usize;
                t.view = t.document.viewport(t.view.chars.start, count);
            } else {
                t.view = t
                    .document
                    .viewport(t.document.selection.head.saturating_sub(2000), WINDOW_CHARS);
            }
            let byte = |p: usize| {
                t.view
                    .text
                    .chars()
                    .take(p.saturating_sub(t.view.chars.start))
                    .map(char::len_utf8)
                    .sum::<usize>() as i32
            };
            let (anchor, head) = (
                byte(t.document.selection.anchor),
                byte(t.document.selection.head),
            );
            a.refresh(ui, true);
            drop(a);
            if ui.get_mode() == 0 || ui.get_mode() == 1 || ui.get_editor_active() {
                ui.invoke_select_source(anchor, head);
            }
            schedule_save(app, ui);
        }
        "export-html" | "export-pdf" => {
            let pdf = name == "export-pdf";
            let ext = if pdf { "pdf" } else { "html" };
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name(format!(
                    "{}.{}",
                    app.borrow().tab().name.trim_end_matches(".md"),
                    ext
                ))
                .add_filter(tr("导出文档"), &[ext])
                .save_file()
            {
                let a = app.borrow();
                let t = a.tab();
                let snapshot = t.document.snapshot();
                let root = t
                    .path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(Path::to_path_buf);
                let weak = ui.as_weak();
                ui.set_notice(tr("正在导出完整文档…").into());
                std::thread::spawn(move || {
                    let message = worker::export(&snapshot.to_string(), &path, root, pdf)
                        .map(|_| message("已导出：{path}", &[("path", path.display().to_string())]))
                        .unwrap_or_else(|e| {
                            message("导出失败：{error}", &[("error", tr(&e.to_string()))])
                        });
                    let _ = weak.upgrade_in_event_loop(move |ui| ui.set_notice(message.into()));
                });
            }
        }
        "render-failed" => {
            app.borrow_mut().render_key = None;
            return;
        }
        "copy-reading" => {
            app.borrow().render.copy();
            return;
        }
        "dismiss" => ui.set_notice("".into()),
        _ => {}
    }
    schedule_render(app, ui);
}
fn try_close_tab(app: &State, ui: &AppWindow, index: usize) -> bool {
    let mut a = app.borrow_mut();
    if index >= a.tabs.len() {
        return true;
    }
    let t = &mut a.tabs[index];
    if t.pending_save.is_some() {
        ui.set_notice(tr("正在保存，请稍后关闭").into());
        return false;
    }
    if t.document.is_dirty() {
        let answer = rfd::MessageDialog::new()
            .set_title(tr("保留修改"))
            .set_description(message(
                "保存「{name}」的修改后关闭？",
                &[("name", t.name.clone())],
            ))
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show();
        if answer == rfd::MessageDialogResult::Cancel {
            return false;
        }
        if answer == rfd::MessageDialogResult::Yes {
            let Some(path) = t
                .path
                .clone()
                .or_else(|| rfd::FileDialog::new().set_file_name(&t.name).save_file())
            else {
                return false;
            };
            if let Err(e) = LocalStorage::save(&path, &t.document.snapshot(), t.stamp.as_ref()) {
                ui.set_notice(tr(&e.to_string()).into());
                return false;
            }
        }
    }
    let _ = std::fs::remove_file(&t.recovery);
    a.persist();
    a.tabs.remove(index);
    if !a.tabs.is_empty() {
        a.active = a.active.min(a.tabs.len() - 1);
    }
    true
}

fn ensure_document(app: &State, ui: &AppWindow) {
    let mut a = app.borrow_mut();
    if a.tabs.is_empty() {
        a.add("", None, None, tr("未命名.md"));
    }
    a.refresh(ui, true);
}

fn close_tab(app: &State, ui: &AppWindow, index: usize) -> bool {
    if !try_close_tab(app, ui, index) {
        return false;
    }
    ensure_document(app, ui);
    true
}

fn close_all_tabs(app: &State, ui: &AppWindow) -> bool {
    let mut index = app.borrow().tabs.len();
    while index > 0 {
        index -= 1;
        if !try_close_tab(app, ui, index) {
            ensure_document(app, ui);
            return false;
        }
    }
    ensure_document(app, ui);
    true
}

#[cfg(target_os = "macos")]
pub(crate) fn open_paths(app: &State, ui: &AppWindow, paths: Vec<PathBuf>) {
    let mut a = app.borrow_mut();
    for path in paths {
        if is_markdown(&path) {
            a.open(path, ui);
        }
    }
    if a.showing_recent {
        a.files = a.prefs.recent.clone();
        a.show_files(ui);
    }
    drop(a);
    let _ = ui.show();
    schedule_render(app, ui);
}

pub fn setup(ui: &AppWindow, args: &[String]) -> anyhow::Result<State> {
    let data = std::env::var_os("HUI_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            directories::ProjectDirs::from("dev", "hui", "HUI")
                .expect("application data directory")
                .data_local_dir()
                .to_path_buf()
        });
    std::fs::create_dir_all(&data)?;
    let prefs = Preferences::load(&data.join("preferences.json"));
    i18n::install(ui, &prefs.language);
    ui.set_theme(prefs.theme);
    ui.set_source_font_size(prefs.source_font_size.clamp(11., 24.));
    ui.set_line_height(prefs.line_height.clamp(1.2, 2.4));
    ui.set_reading_width(prefs.reading_width.clamp(520., 1040.));
    ui.set_paragraph_spacing(prefs.paragraph_spacing.clamp(0., 40.));
    ui.set_font_size(prefs.font_size.clamp(13., 28.));
    let next_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let app = Rc::new(RefCell::new(App {
        tabs: vec![],
        active: 0,
        next_id,
        files: vec![],
        showing_recent: true,
        prefs,
        data,
        render: worker::Renderer::new(ui.as_weak()),
        render_key: None,
        clipboard: None,
        debounce: Timer::default(),
        autosave: Timer::default(),
        search_cancel: Arc::new(AtomicBool::new(false)),
    }));
    {
        let mut a = app.borrow_mut();
        a.add(i18n::welcome(), None, None, tr("欢迎使用 HUI") + ".md");
        for arg in args {
            a.open(PathBuf::from(arg), ui);
        }
        let recovery = std::fs::read_dir(&a.data)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("recovery-"))
                    && is_markdown(p)
            })
            .collect::<Vec<_>>();
        for p in recovery {
            if let Ok((text, _)) = LocalStorage::read(&p) {
                a.add("", None, None, tr("恢复的文档.md"));
                let t = a.tab_mut();
                t.document.apply(
                    0,
                    vec![hui_core::Edit {
                        range: 0..0,
                        insert: text.to_string(),
                    }],
                )?;
                t.recovery = p;
                t.view = t.document.viewport(0, WINDOW_CHARS);
                ui.set_notice(tr("已恢复上次未保存的文档，请检查后另存为").into());
            }
        }
        a.files = a.prefs.recent.clone();
        a.show_files(ui);
        a.refresh(ui, true);
    }
    bind(ui, &app);
    Ok(app)
}

fn bind(ui: &AppWindow, app: &State) {
    ui.on_highlight_source(|text, dark| {
        let markup = hui_render::markdown::highlighted_source(&text, dark);
        if markup.is_empty() {
            return slint::StyledText::from_plain_text(&text);
        }
        slint::StyledText::from_markdown(&markup)
            .unwrap_or_else(|_| slint::StyledText::from_plain_text(&text))
    });
    ui.set_code_font(
        if cfg!(target_os = "macos") {
            "Menlo"
        } else if cfg!(target_os = "windows") {
            "Consolas"
        } else {
            "DejaVu Sans Mono"
        }
        .into(),
    );
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_action(move |name| {
        if let Some(ui) = w.upgrade() {
            action(&a, &ui, &name);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_edit(move |text| {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            let t = app.tab_mut();
            if let Err(e) = t.document.replace_viewport(&t.view, &text) {
                ui.set_notice(tr(&e.to_string()).into());
                app.refresh(&ui, true);
                return;
            }
            t.view = t
                .document
                .viewport(t.view.chars.start, text.chars().count());
            app.refresh(&ui, false);
            drop(app);
            schedule_render(&a, &ui);
            schedule_save(&a, &ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_resize_preview(move || {
        if let Some(ui) = w.upgrade() {
            schedule_render(&a, &ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_language_changed(move |index| {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            app.prefs.language = index
                .checked_sub(1)
                .and_then(|i| i18n::LANGUAGES.get(i as usize))
                .copied()
                .unwrap_or("system")
                .into();
            i18n::apply(&ui, &app.prefs.language);
            if app.showing_recent {
                ui.set_folder_name(tr("最近打开").into());
            }
            ui.set_notice("".into());
            ui.set_search_result("".into());
            app.persist();
            app.refresh(&ui, false);
            drop(app);
            schedule_render(&a, &ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_appearance(move || {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            app.prefs.theme = ui.get_theme();
            app.prefs.font_size = ui.get_font_size();
            app.prefs.source_font_size = ui.get_source_font_size();
            app.prefs.line_height = ui.get_line_height();
            app.prefs.paragraph_spacing = ui.get_paragraph_spacing();
            app.prefs.reading_width = ui.get_reading_width();
            app.persist();
            drop(app);
            schedule_render(&a, &ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_step(move |direction| {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            let t = app.tab();
            let pos = if direction < 0 {
                t.view.chars.start.saturating_sub(WINDOW_CHARS)
            } else {
                t.view.chars.end
            };
            app.navigate(pos, &ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_navigate(move |position| {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            let pos = (app.tab().document.len_chars() as f32 * position) as usize;
            app.navigate(pos, &ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_file_clicked(move |index| {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            if let Some(path) = app.files.get(index as usize).cloned() {
                if path.is_dir() {
                    app.folder(&path, &ui);
                } else {
                    app.open(path, &ui);
                }
            }
            drop(app);
            schedule_render(&a, &ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_tab_clicked(move |index| {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            if (index as usize) < app.tabs.len() {
                app.active = index as usize;
                ui.set_editor_active(false);
                app.refresh(&ui, true);
            }
            drop(app);
            ui.set_preview_position(0.);
            a.borrow_mut().request_render(&ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_tab_closed(move |index| {
        if let Some(ui) = w.upgrade() {
            close_tab(&a, &ui, index as usize);
            a.borrow_mut().request_render(&ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_scroll_source(move |progress| {
        if let Some(ui) = w.upgrade() {
            let y = progress.clamp(0., 1.) * ui.get_preview_max().max(0.);
            ui.set_preview_position(y);
            a.borrow().render.scroll(y);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_scroll_preview(move |delta| {
        if let Some(ui) = w.upgrade() {
            let y = (ui.get_preview_position() + delta).clamp(0., ui.get_preview_max().max(0.));
            ui.set_preview_position(y);
            if ui.get_preview_max() > 0. {
                ui.invoke_sync_source_scroll(y / ui.get_preview_max());
            }
            a.borrow().render.scroll(y);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_open_inline(move |x, y| {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            if ui.get_editor_active() && app.render_key != Some(app.current_render_key(&ui)) {
                // Resolve the click in the layout the user can still see, rebuild underneath
                // the old editor, then replace editor and preview in one UI update.
                app.render.prepare_inline_switch(x, y);
                app.request_render_inner(&ui, true);
            } else {
                // The source is unchanged, so the native document can switch blocks directly.
                app.render.hit(x, y);
            }
        }
    });
    let a = app.clone();
    ui.on_select_reading(move |phase, x, y| {
        a.borrow().render.select(phase, x, y);
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_copy_ready(move |text| {
        let mut app = a.borrow_mut();
        let result = (|| -> anyhow::Result<()> {
            if app.clipboard.is_none() {
                app.clipboard = Some(arboard::Clipboard::new()?);
            }
            app.clipboard.as_mut().unwrap().set_text(text.to_string())?;
            Ok(())
        })();
        if let Err(e) = result
            && let Some(ui) = w.upgrade()
        {
            ui.set_notice(message("复制失败：{error}", &[("error", tr(&e.to_string()))]).into());
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_inline_ready(move |font, caret, start, end, x, y, width, height| {
        let Some(ui) = w.upgrade() else {
            return;
        };
        if ui.get_mode() != 2 {
            return;
        }
        let mut app = a.borrow_mut();
        let t = app.tab_mut();
        let rope = t.document.snapshot();
        let start = rope.byte_to_char((start.max(0) as usize).min(rope.len_bytes()));
        let mut end = rope.byte_to_char((end.max(0) as usize).min(rope.len_bytes()));
        // Keep the terminating line ending in the Rope, outside the editable viewport.
        // Displaying it creates a spurious empty line and moves every following block.
        if end > start && rope.char(end - 1) == '\n' {
            end -= 1;
            if end > start && rope.char(end - 1) == '\r' {
                end -= 1;
            }
        }
        if end.saturating_sub(start) > WINDOW_CHARS {
            app.navigate(start, &ui);
            ui.set_mode(0);
            ui.set_notice(tr("此块较长，已定位到源码视口").into());
            return;
        }
        let block_source = rope.slice(start..end).to_string();
        let trimmed = block_source.trim();
        let inline_math = (trimmed.starts_with("$$") && trimmed.ends_with("$$"))
            || (trimmed.starts_with("\\[") && trimmed.ends_with("\\]"));
        t.view = t.document.viewport(start, end.saturating_sub(start));
        app.refresh(&ui, true);
        ui.set_inline_x(x);
        ui.set_inline_y(y);
        ui.set_inline_width(width);
        ui.set_inline_height(height);
        ui.set_inline_font_size(font);
        ui.set_inline_math(inline_math);
        if inline_math {
            app.request_formula_preview(&ui);
        } else {
            ui.set_inline_formula_preview(slint::Image::default());
        }
        ui.invoke_reset_inline_scroll();
        ui.set_editor_active(true);
        let caret = caret.clamp(0, ui.get_source().len() as i32);
        ui.invoke_select_source(caret, caret);
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_heading_clicked(move |index| {
        if let Some(ui) = w.upgrade()
            && let Some(h) = ui.get_headings().row_data(index as usize)
        {
            let mut app = a.borrow_mut();
            let t = app.tab();
            let pos = t
                .document
                .snapshot()
                .byte_to_char((h.offset as usize).min(t.document.len_bytes()));
            app.navigate(pos, &ui);
            app.render.heading(h.offset as usize);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_save_finished(move |id, stamp, error| {
        if let Some(ui) = w.upgrade() {
            let mut app = a.borrow_mut();
            if let Some(t) = app
                .tabs
                .iter_mut()
                .find(|t| t.id.to_string() == id.as_str())
                && let Some((path, snapshot)) = t.pending_save.take()
            {
                if error.is_empty() && path == t.recovery {
                    if t.document.snapshot() == snapshot {
                        t.recovery_revision = t.document.revision();
                    }
                } else if error.is_empty() {
                    t.document.mark_saved(snapshot);
                    t.stamp = stamp.parse().ok().map(FileStamp);
                    t.name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    t.path = Some(path);
                    if !t.document.is_dirty() {
                        let _ = std::fs::remove_file(&t.recovery);
                    }
                } else {
                    ui.set_notice(error.clone());
                }
            }
            let retry = error.is_empty()
                && app.tabs.iter().any(|t| {
                    t.document.is_dirty()
                        && (t.path.is_some() || t.document.revision() != t.recovery_revision)
                });
            app.refresh(&ui, false);
            drop(app);
            if retry {
                schedule_save(&a, &ui);
            }
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_format(move |kind| {
        if let Some(ui) = w.upgrade() {
            if ui.get_mode() == 3 || (ui.get_mode() == 2 && !ui.get_editor_active()) {
                return;
            }
            let mut app = a.borrow_mut();
            app.sync_selection(&ui);
            let t = app.tab_mut();
            let s = &t.document.selection;
            let range = s.anchor.min(s.head)..s.anchor.max(s.head);
            let Some(result) = hui_core::format::command(&t.document.snapshot(), range, &kind)
            else {
                return;
            };
            let old_len = t.document.len_chars();
            let view_start = t.view.chars.start.min(result.edit.range.start);
            if t.document
                .apply(t.document.revision(), vec![result.edit])
                .is_err()
            {
                return;
            }
            t.document.selection = result.selection.clone();
            let count = if ui.get_mode() == 2 {
                (t.view.chars.len() as isize + t.document.len_chars() as isize - old_len as isize)
                    .max(0) as usize
            } else {
                WINDOW_CHARS
            };
            t.view = t.document.viewport(view_start, count);
            let byte = |p: usize| {
                t.view
                    .text
                    .chars()
                    .take(p.saturating_sub(view_start))
                    .map(char::len_utf8)
                    .sum::<usize>() as i32
            };
            let (anchor, head) = (byte(result.selection.anchor), byte(result.selection.head));
            app.refresh(&ui, true);
            drop(app);
            ui.invoke_select_source(anchor, head);
            schedule_render(&a, &ui);
            schedule_save(&a, &ui);
        }
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_shortcut(move |key, shift| {
        let Some(ui) = w.upgrade() else {
            return false;
        };
        let key = key.to_ascii_lowercase();
        let format = match (key.as_str(), shift) {
            ("b", _) => Some("bold"),
            ("i", _) => Some("italic"),
            ("k", false) => Some("link"),
            ("e", false) => Some("code"),
            ("e", true) => Some("code-block"),
            ("x", true) => Some("strike"),
            ("1", false) => Some("h1"),
            ("2", false) => Some("h2"),
            ("3", false) => Some("h3"),
            ("4", false) => Some("h4"),
            ("5", false) => Some("h5"),
            ("6", false) => Some("h6"),
            ("8" | "*", true) => Some("list"),
            ("7" | "&", true) => Some("ordered"),
            ("9" | "(", true) => Some("quote"),
            (" ", true) => Some("task"),
            _ => None,
        };
        if let Some(kind) = format {
            ui.invoke_format(kind.into());
            return true;
        }
        if key == "z" && shift {
            action(&a, &ui, "redo");
            return true;
        }
        let command = match key.to_ascii_lowercase().as_str() {
            "c" => "copy-source",
            "x" => "cut-source",
            "v" => "paste-source",
            "a" => {
                ui.invoke_select_source(0, ui.get_source().len() as i32);
                return true;
            }
            "s" => "save",
            "o" => "open",
            "n" => "new",
            "z" => "undo",
            "y" => "redo",
            "f" => {
                ui.set_search_visible(!ui.get_search_visible());
                return true;
            }
            "k" => {
                ui.set_commands_visible(!ui.get_commands_visible());
                return true;
            }
            "b" => {
                ui.invoke_format("bold".into());
                return true;
            }
            "i" => {
                ui.invoke_format("italic".into());
                return true;
            }
            _ => return false,
        };
        action(&a, &ui, command);
        true
    });
    bind_search(ui, app);
    let a = app.clone();
    let w = ui.as_weak();
    ui.window().on_close_requested(move || {
        let Some(ui) = w.upgrade() else {
            return slint::CloseRequestResponse::HideWindow;
        };
        let count = a.borrow().tabs.len();
        for _ in 0..count {
            if !close_tab(&a, &ui, 0) {
                return slint::CloseRequestResponse::KeepWindowShown;
            }
        }
        slint::CloseRequestResponse::HideWindow
    });
}

fn bind_search(ui: &AppWindow, app: &State) {
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_search(move |query, replacement, regex, replace| {
        let Some(ui) = w.upgrade() else {
            return;
        };
        let mut app = a.borrow_mut();
        app.search_cancel.store(true, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        app.search_cancel = cancel.clone();
        let t = app.tab();
        let text = t.document.snapshot();
        let id = t.id.to_string();
        let rev = t.document.revision().to_string();
        let weak = ui.as_weak();
        ui.set_search_result(tr("正在搜索全文…").into());
        std::thread::spawn(move || {
            let result: anyhow::Result<String> = if replace {
                hui_core::search::replace_all(&text, &query, &replacement, regex, false)
                    .map(|edits| {
                        serde_json::to_string(
                            &edits
                                .iter()
                                .map(|e| (e.range.start, e.range.end, &e.insert))
                                .collect::<Vec<_>>(),
                        )
                        .unwrap()
                    })
                    .map_err(Into::into)
            } else {
                hui_core::search::find(&text, &query, regex, false, &cancel, 10000)
                    .map(|ranges| {
                        serde_json::to_string(
                            &ranges
                                .iter()
                                .map(|r| (r.start, r.end, ""))
                                .collect::<Vec<_>>(),
                        )
                        .unwrap()
                    })
                    .map_err(Into::into)
            };
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            match result {
                Ok(payload) => {
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        ui.invoke_search_finished(id.into(), rev.into(), payload.into(), replace)
                    });
                }
                Err(e) => {
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        ui.set_search_result(
                            message("搜索失败：{error}", &[("error", tr(&e.to_string()))]).into(),
                        )
                    });
                }
            }
        });
    });
    let a = app.clone();
    let w = ui.as_weak();
    ui.on_search_finished(move |id, revision, payload, replace| {
        let Some(ui) = w.upgrade() else {
            return;
        };
        let mut app = a.borrow_mut();
        let t = app.tab_mut();
        if t.id.to_string() != id.as_str() || t.document.revision().to_string() != revision.as_str()
        {
            ui.set_search_result(tr("内容已变化，请重新搜索").into());
            return;
        }
        let results: Vec<(usize, usize, String)> =
            serde_json::from_str(&payload).unwrap_or_default();
        ui.set_search_result(
            message(
                "匹配数量：{count}{limit}",
                &[
                    ("count", results.len().to_string()),
                    (
                        "limit",
                        if results.len() == 10000 {
                            tr("（显示前 10000 个）")
                        } else {
                            String::new()
                        },
                    ),
                ],
            )
            .into(),
        );
        if replace {
            let edits = results
                .iter()
                .map(|(s, e, text)| hui_core::Edit {
                    range: *s..*e,
                    insert: text.clone(),
                })
                .collect();
            if let Err(e) = t.document.apply(t.document.revision(), edits) {
                ui.set_notice(tr(&e.to_string()).into());
                return;
            }
            t.view = t.document.viewport(t.view.chars.start, WINDOW_CHARS);
            app.refresh(&ui, true);
        } else if let Some((start, end, _)) = results.first() {
            app.navigate(*start, &ui);
            ui.set_mode(0);
            let bytes = app
                .tab()
                .view
                .text
                .chars()
                .take(end - start)
                .map(char::len_utf8)
                .sum::<usize>();
            ui.invoke_select_source(0, bytes as i32);
        }
        drop(app);
        if replace {
            schedule_render(&a, &ui);
            schedule_save(&a, &ui);
        }
    });
}
