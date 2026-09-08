mod controller;
mod i18n;
#[cfg(target_os = "macos")]
mod macos_open;
mod worker;
use slint::ComponentHandle;
slint::include_modules!();

fn main() -> anyhow::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|s| s == "--render") {
        return worker::render_cli(&args[1..]);
    }
    let capture = args.first().is_some_and(|s| s == "--screenshot");
    if capture {
        slint::BackendSelector::new()
            .renderer_name("software".into())
            .select()?;
    }
    let ui = AppWindow::new()?;
    let app = controller::setup(&ui, if capture { &[] } else { &args })?;
    #[cfg(target_os = "macos")]
    if !capture {
        macos_open::install(&app, &ui)?;
    }
    let mut snapshot_timer = None;
    if capture {
        let path = std::path::PathBuf::from(
            args.get(1)
                .ok_or_else(|| anyhow::anyhow!("Missing screenshot output path"))?,
        );
        if args.iter().any(|a| a == "dark") {
            ui.set_theme(1);
        }
        if args.iter().any(|a| a == "paper") {
            ui.set_theme(2);
        }
        if args.iter().any(|a| a == "split") {
            ui.set_mode(1);
        }
        if args.iter().any(|a| a == "source") {
            ui.set_mode(0);
        }
        if args.iter().any(|a| a == "settings") {
            ui.set_settings_visible(true);
        }
        if args.iter().any(|a| a == "compact") {
            ui.set_initial_width(430.);
            ui.set_initial_height(820.);
            ui.window().set_size(slint::LogicalSize::new(430., 820.));
        }
        let selection = args.iter().any(|a| a == "source-selection");
        if selection {
            ui.set_mode(0);
        }
        let instant = args.iter().any(|a| a == "instant");
        if instant {
            ui.set_mode(2);
        }
        let activated = std::cell::Cell::new(false);
        let timer = slint::Timer::default();
        let weak = ui.as_weak();
        let started = std::time::Instant::now();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_secs(1),
            move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if started.elapsed().as_secs() < 3
                    || (ui.get_busy() && started.elapsed().as_secs() < 30)
                {
                    return;
                }
                if selection && !activated.replace(true) {
                    let source = ui.get_source();
                    let offset = source.find("HUI").unwrap_or(0) as i32;
                    ui.invoke_select_source(offset, offset + 3);
                    return;
                }
                if instant && !activated.replace(true) {
                    ui.invoke_open_inline(150., 118.);
                    return;
                }
                let result = ui
                    .window()
                    .take_snapshot()
                    .map_err(anyhow::Error::from)
                    .and_then(|pixels| {
                        hui_render::native::write_png(
                            &path,
                            pixels.width(),
                            pixels.height(),
                            pixels.as_bytes(),
                        )
                    });
                if let Err(e) = result {
                    eprintln!("screenshot: {e}");
                }
                let _ = slint::quit_event_loop();
            },
        );
        snapshot_timer = Some(timer);
    }
    ui.show()?;
    if capture && args.iter().any(|a| a == "compact") {
        ui.window().set_size(slint::LogicalSize::new(430., 820.));
    }
    controller::schedule_render(&app, &ui);
    slint::run_event_loop()?;
    app.borrow_mut().persist();
    drop(snapshot_timer);
    Ok(())
}
