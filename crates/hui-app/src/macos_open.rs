use crate::{AppWindow, controller};
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{ffi, sel};
use objc2_foundation::{NSArray, NSString};
use slint::ComponentHandle;
use std::{cell::RefCell, ffi::c_char, path::PathBuf};

type OpenHandler = Box<dyn Fn(Vec<PathBuf>)>;

thread_local! {
    static OPEN_HANDLER: RefCell<Option<OpenHandler>> = const { RefCell::new(None) };
}

unsafe extern "C-unwind" fn application_open_files(
    _delegate: &AnyObject,
    _selector: Sel,
    application: &AnyObject,
    filenames: &NSArray<NSString>,
) {
    let paths = filenames
        .iter()
        .map(|filename| PathBuf::from(filename.to_string()))
        .collect::<Vec<_>>();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        OPEN_HANDLER.with_borrow(|handler| {
            if let Some(handler) = handler.as_ref() {
                handler(paths);
            }
        });
    }));
    let _: () = unsafe { objc2::msg_send![application, replyToOpenOrPrint: 0usize] };
}

pub fn install(app: &controller::State, ui: &AppWindow) -> anyhow::Result<()> {
    let state = app.clone();
    let weak = ui.as_weak();
    OPEN_HANDLER.with_borrow_mut(|handler| {
        *handler = Some(Box::new(move |paths| {
            if let Some(ui) = weak.upgrade() {
                controller::open_paths(&state, &ui, paths);
            }
        }));
    });

    let class = AnyClass::get(c"WinitApplicationDelegate")
        .ok_or_else(|| anyhow::anyhow!("找不到 macOS 应用委托"))?;
    let method: Imp = unsafe {
        std::mem::transmute::<
            unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSString>),
            Imp,
        >(application_open_files)
    };
    let signature = b"v@:@@\0";
    let added = unsafe {
        ffi::class_addMethod(
            (class as *const AnyClass).cast_mut(),
            sel!(application:openFiles:),
            method,
            signature.as_ptr().cast::<c_char>(),
        )
    };
    if !added.as_bool() {
        anyhow::bail!("无法注册 macOS 打开文稿处理器");
    }
    Ok(())
}
