//! Android OS integration. Document bytes stay in app-private mirrors; Java owns SAF grants.
use crate::{AppWindow, controller};
use jni::{
    JNIEnv, JavaVM,
    objects::{GlobalRef, JObject, JString, JValue},
};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
static JAVA: OnceLock<JavaVM> = OnceLock::new();
static ACTIVITY: Mutex<Option<GlobalRef>> = Mutex::new(None);
static DATA: OnceLock<PathBuf> = OnceLock::new();
thread_local! { static UI: RefCell<Option<(slint::Weak<AppWindow>, controller::State)>> = const { RefCell::new(None) }; }
pub fn data_dir() -> PathBuf {
    DATA.get().expect("Android initialized").clone()
}
pub fn call(method: &str, args: &[&str]) -> anyhow::Result<String> {
    let vm = JAVA
        .get()
        .ok_or_else(|| anyhow::anyhow!("Android VM unavailable"))?;
    let activity = ACTIVITY
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Android activity unavailable"))?;
    let mut env = vm.attach_current_thread()?;
    // Calls also originate on long-lived native threads; bound JNI local references.
    env.with_local_frame(16, |env| {
        let strings = args
            .iter()
            .map(|s| env.new_string(s).map(JObject::from))
            .collect::<Result<Vec<_>, _>>()?;
        let values = strings.iter().map(JValue::Object).collect::<Vec<_>>();
        let signature = format!(
            "({})Ljava/lang/String;",
            "Ljava/lang/String;".repeat(args.len())
        );
        let result = env.call_method(activity.as_obj(), method, signature, &values);
        if env.exception_check()? {
            env.exception_clear()?;
            anyhow::bail!("Android {method} failed");
        }
        let obj = result?.l()?;
        if obj.is_null() {
            return Ok(String::new());
        }
        Ok(env.get_string(&JString::from(obj))?.into())
    })
}
pub fn sync_document(path: &Path) -> anyhow::Result<()> {
    let error = call("syncDocument", &[&path.to_string_lossy()])?;
    anyhow::ensure!(error.is_empty(), "{error}");
    Ok(())
}
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    if let Err(e) = launch(app) {
        eprintln!("HUI Android: {e:#}");
    }
}
fn launch(app: slint::android::AndroidApp) -> anyhow::Result<()> {
    // AndroidApp owns the VM/activity references for the full native activity lifetime.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast())? };
    let activity = {
        let env = vm.attach_current_thread()?;
        env.new_global_ref(unsafe { JObject::from_raw(app.activity_as_ptr().cast()) })?
    };
    let _ = JAVA.set(vm);
    *ACTIVITY.lock().unwrap() = Some(activity);
    DATA.set(PathBuf::from(call("dataDirectory", &[])?)).ok();
    slint::android::init(app)?;
    let ui = AppWindow::new()?;
    {
        use slint::fontique_010::{fontique, shared_collection};
        let mut collection = shared_collection();
        let fonts = collection.register_fonts(
            fontique::Blob::new(hui_render::native::android_font_bytes()),
            None,
        );
        anyhow::ensure!(!fonts.is_empty(), "Invalid bundled CJK font");
        for script in ["Hani", "Hira", "Kana", "Hang"] {
            collection.set_fallbacks(
                fontique::FallbackKey::new(fontique::Script::from_str_unchecked(script), None),
                fonts.iter().map(|font| font.0),
            );
        }
    }
    ui.set_ui_font("Noto Sans CJK SC".into());
    ui.set_mobile(true);
    ui.set_sidebar_visible(false);
    ui.set_mode(3);
    let state = controller::setup(&ui, &[])?;
    UI.with(|slot| *slot.borrow_mut() = Some((ui.as_weak(), state.clone())));
    ui.show()?;
    controller::schedule_render(&state, &ui);
    call("nativeReady", &[])?;
    slint::run_event_loop()?;
    controller::checkpoint(&state);
    UI.with(|slot| slot.borrow_mut().take());
    ACTIVITY.lock().unwrap().take();
    Ok(())
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_hui_markdown_HuiActivity_nativeEvent(
    mut env: JNIEnv,
    _activity: JObject,
    kind: JString,
    value: JString,
) {
    let Ok(kind) = env.get_string(&kind).map(String::from) else {
        return;
    };
    let Ok(value) = env.get_string(&value).map(String::from) else {
        return;
    };
    dispatch(kind, value);
}
pub fn dispatch(kind: String, value: String) {
    let _ = slint::invoke_from_event_loop(move || {
        UI.with(|slot| {
            let slot = slot.borrow();
            let Some((weak, state)) = slot.as_ref() else {
                return;
            };
            let Some(ui) = weak.upgrade() else {
                return;
            };
            match kind.as_str() {
                "open" => {
                    controller::open_paths(state, &ui, vec![PathBuf::from(value)]);
                    ui.set_mode(3);
                }
                "pause" => controller::checkpoint(state),
                "saved" | "cancel-save" => controller::android_saved(
                    state,
                    &ui,
                    PathBuf::from(value),
                    kind == "cancel-save",
                ),
                _ => ui.set_notice(value.into()),
            }
        })
    });
}
pub mod clipboard {
    pub struct Clipboard;
    pub struct ImageData {
        pub width: usize,
        pub height: usize,
        pub bytes: Vec<u8>,
    }
    impl Clipboard {
        pub fn new() -> anyhow::Result<Self> {
            Ok(Self)
        }
        pub fn set_text(&mut self, text: String) -> anyhow::Result<()> {
            super::call("copyText", &[&text]).map(|_| ())
        }
        pub fn get_text(&mut self) -> anyhow::Result<String> {
            super::call("pasteText", &[])
        }
        pub fn get_image(&mut self) -> anyhow::Result<ImageData> {
            anyhow::bail!("Android clipboard image insertion is not supported yet")
        }
    }
}

// In-process instrumentation probe, like the desktop --screenshot path. No OS capture.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_hui_markdown_HuiActivity_nativeDisplayProbe(
    env: JNIEnv,
    _activity: JObject,
) -> jni::sys::jstring {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let _ = slint::invoke_from_event_loop(move || {
        UI.with(|slot| {
            let slot = slot.borrow();
            let Some((weak, _)) = slot.as_ref() else {
                return;
            };
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = ui
                .window()
                .take_snapshot()
                .map_err(anyhow::Error::from)
                .and_then(|pixels| {
                    hui_render::native::write_png(
                        &data_dir().join("display-probe.png"),
                        pixels.width(),
                        pixels.height(),
                        pixels.as_bytes(),
                    )
                });
            let size = ui.window().size().to_logical(ui.window().scale_factor());
            let _ = send.send(
                serde_json::json!({
                    "top": ui.get_mobile_safe_top(), "bottom": ui.get_mobile_safe_bottom(),
                    "left": ui.get_mobile_safe_left(), "right": ui.get_mobile_safe_right(),
                    "width": size.width, "height": size.height,
                    "font": ui.get_ui_font().as_str(), "snapshot": result.is_ok(),
                    "error": result.err().map(|e| e.to_string())
                })
                .to_string(),
            );
        });
    });
    let value = receive
        .recv_timeout(std::time::Duration::from_secs(15))
        .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}).to_string());
    env.new_string(value)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
