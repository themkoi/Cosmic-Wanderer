use shlex::Shlex;
use slint::{set_xdg_app_id, ModelRc};
use std::{error::Error, process::Command, rc::Rc, thread, time, time::Instant};
mod entries;
use entries::DesktopEntryManager;
use slint::Image;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    let manager = DesktopEntryManager::new();
    let normalized_entries = manager.get_normalized_entries();
    let _ = set_xdg_app_id("Cosmic Wanderer");

    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();

    let slint_items = slint::VecModel::from(
        normalized_entries
            .iter()
            .map(|entry| AppItem {
                appid: entry.appid.clone().into(),
                icon: Image::load_from_path(entry.icon.as_ref()).unwrap_or_else(|_| Image::default()),
            })
            .collect::<Vec<_>>(),
    );

    ui.set_appItems(ModelRc::new(Rc::new(slint_items)));

    // Focus handling thread
    let ui_weak_clone = ui_weak.clone();
    thread::spawn(move || {
        loop {
            slint::invoke_from_event_loop({
                let ui_weak = ui_weak_clone.clone();
                move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let scope_focused: bool = ui.get_scopeFocused();
                        if !scope_focused {
                            ui.invoke_readFocus();
                        }
                    }
                }
            })
            .unwrap();
            thread::sleep(time::Duration::from_millis(500));
        }
    });

    // Text input handler
    ui.on_text_entered(move |text| {
        println!("Search: {}", text);
    });

    let ui_weak_clone_item = ui_weak.clone();
    ui.on_item_clicked(move |idx| {
        let idx = idx as usize;
        let entry = &normalized_entries[idx];

        let command_string = entry
            .exec
            .replace("%U", "")
            .replace("%F", "")
            .replace("%u", "")
            .replace("%f", "");

        let command: Vec<String> = Shlex::new(&command_string).collect();

        if let Some((cmd, args)) = command.split_first() {
            let _ = Command::new(cmd)
                .args(args)
                .spawn()
                .expect("Failed to launch app");
        }

        if let Some(ui) = ui_weak_clone_item.upgrade() {
            ui.hide().unwrap();
        }
    });

    // Focus change handler
    ui.on_focus_changed(move |focused| {
        if !focused {
            if let Some(ui) = ui_weak.upgrade() {
                ui.hide().unwrap();
            }
        }
    });

    ui.invoke_focusText();
    println!("Time taken: {:?}", start.elapsed());
    ui.run()?;
    Ok(())
}
