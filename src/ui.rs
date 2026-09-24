//! GTK4 + libadwaita user interface: history list, search, row menu,
//! preferences, and clipboard capture wiring.

use crate::backend::{spawn_wayland_watcher, write_clipboard, WatchEvent};
use crate::format::{now_ts, preview_text, rel_time};
use crate::models::{load_history, save_history, save_settings, ClipItem, CloseAction, Settings};
use crate::paths::{
    autostart_path, config_dir, desktop_entry, exe_path, history_path, socket_path,
};
use adw::prelude::*;
use gtk4::{gio, glib, glib::ControlFlow};
use libadwaita as adw;
use std::cell::RefCell;
use std::io::Read;
use std::os::unix::net::UnixListener;
use std::rc::Rc;

// State

const MAX_ITEM_BYTES: usize = 200_000;

pub struct AppState {
    pub items: Vec<ClipItem>, // oldest first, newest at the end
    pub query: String,
    pub settings: Settings,
    pub last_clip: String,
}

impl AppState {
    /// Display order: pinned first, then newest first, filtered by query.
    pub fn display_indices(&self) -> Vec<usize> {
        let q = self.query.trim().to_lowercase();
        let mut idx: Vec<usize> = (0..self.items.len())
            .filter(|&i| q.is_empty() || self.items[i].text.to_lowercase().contains(&q))
            .collect();
        idx.sort_by(|&a, &b| {
            let (ia, ib) = (&self.items[a], &self.items[b]);
            ib.pinned.cmp(&ia.pinned).then(
                ib.ts
                    .partial_cmp(&ia.ts)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        idx
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(config_dir());
        save_history(&self.items);
        save_settings(&self.settings);
    }

    pub fn apply_autostart(&self) {
        if self.settings.autostart {
            if let Some(dir) = autostart_path().parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(autostart_path(), desktop_entry(&exe_path()));
        } else {
            let _ = std::fs::remove_file(autostart_path());
        }
    }

    pub fn add_clip(&mut self, text: String) {
        let ts = now_ts();
        if let Some(pos) = self.items.iter().position(|i| i.text == text) {
            let pinned = self.items[pos].pinned;
            self.items[pos].ts = ts;
            if !pinned {
                let item = self.items.remove(pos);
                self.items.push(item); // re-copied → newest
            }
        } else {
            self.items.push(ClipItem {
                text,
                pinned: false,
                ts,
            });
        }
        self.trim();
    }

    pub fn trim(&mut self) {
        let max = self.settings.max_items;
        let mut unpinned = self.items.iter().filter(|i| !i.pinned).count();
        let mut i = 0;
        while i < self.items.len() && unpinned > max {
            if !self.items[i].pinned {
                self.items.remove(i);
                unpinned -= 1;
            } else {
                i += 1;
            }
        }
    }

    /// Copy item `idx` back to the clipboard. Returns true on success.
    pub fn copy(&mut self, idx: usize) -> bool {
        let Some(text) = self.items.get(idx).map(|i| i.text.clone()) else {
            return false;
        };
        if !write_clipboard(&text) {
            return false;
        }
        self.last_clip = text;
        let ts = now_ts();
        if let Some(item) = self.items.get_mut(idx) {
            item.ts = ts;
            if !item.pinned {
                let item = self.items.remove(idx);
                self.items.push(item);
            }
        }
        true
    }

    /// Record a newly captured clip, skipping duplicates and oversized text.
    pub fn ingest(&mut self, text: String) {
        if text.trim().is_empty() || text == self.last_clip {
            return;
        }
        if text.len() > MAX_ITEM_BYTES {
            self.last_clip = text;
            return;
        }
        self.last_clip = text.clone();
        self.add_clip(text);
    }
}

/// Copy the first display item; returns whether the window should hide.
fn copy_first(state: &Rc<RefCell<AppState>>) -> bool {
    let mut st = state.borrow_mut();
    let Some(idx) = st.display_indices().first().copied() else {
        return false;
    };
    let ok = st.copy(idx);
    let hide = ok && st.settings.hide_after_copy;
    st.save();
    hide
}

/// Copy the display-indexed item; returns whether the window should hide.
fn copy_display(state: &Rc<RefCell<AppState>>, vi: usize) -> bool {
    let mut st = state.borrow_mut();
    let Some(idx) = st.display_indices().get(vi).copied() else {
        return false;
    };
    let ok = st.copy(idx);
    let hide = ok && st.settings.hide_after_copy;
    st.save();
    hide
}

// Entry point

pub fn main() {
    let app = adw::Application::new(
        Some("io.github.raheem_dotgit.Pastel"),
        gio::ApplicationFlags::empty(),
    );
    app.connect_activate(build_ui);
    std::process::exit(app.run().into());
}

fn build_ui(app: &adw::Application) {
    let settings = Settings::load();
    let listener: Option<UnixListener> = UnixListener::bind(socket_path()).ok();
    if let Some(l) = &listener {
        let _ = l.set_nonblocking(true);
    }
    let listener = std::rc::Rc::new(listener);

    let state = Rc::new(RefCell::new(AppState {
        items: load_history(),
        query: String::new(),
        settings,
        last_clip: String::new(),
    }));

    // Window and header bar
    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Pastel"));
    window.set_default_size(430, 560);

    let header = gtk4::HeaderBar::new();
    let title = adw::WindowTitle::new("Pastel", "");
    header.set_title_widget(Some(&title));

    let menu = gio::Menu::new();
    menu.append(Some("_Preferences"), Some("win.settings"));
    menu.append(Some("_Clear Unpinned"), Some("win.clear"));
    let quit_section = gio::Menu::new();
    quit_section.append(Some("_Quit"), Some("app.quit"));
    menu.append_section(None, &quit_section);

    let menu_btn = gtk4::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.set_menu_model(Some(&menu));
    header.pack_end(&menu_btn);
    // Content. AdwApplicationWindow has no titlebar slot, so the header bar
    // goes at the top of the content box.
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    vbox.append(&header);

    let search = gtk4::SearchEntry::new();
    search.set_placeholder_text(Some("Search clipboard"));
    search.set_hexpand(true);
    search.set_margin_top(8);
    search.set_margin_bottom(6);
    search.set_margin_start(10);
    search.set_margin_end(10);
    vbox.append(&search);

    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::Single);
    list.set_vexpand(true);
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_child(Some(&list));
    scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    vbox.append(&scrolled);
    window.set_content(Some(&vbox));

    // Right-click menu shared by all rows
    let row_menu = gio::Menu::new();
    row_menu.append(Some("_Copy"), Some("win.row-copy"));
    row_menu.append(Some("_Pin/Unpin"), Some("win.row-pin"));
    row_menu.append(Some("_Delete"), Some("win.row-delete"));
    let popover = gtk4::PopoverMenu::from_model(Some(&row_menu));
    popover.set_parent(&window);

    // Display index of the row the menu was opened on
    let popup_vi: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));

    // Rebuild the list from state
    let rebuild: Rc<dyn Fn()> = {
        let state = state.clone();
        let list = list.clone();
        let popover = popover.clone();
        let popup_vi = popup_vi.clone();
        let title = title.clone();
        Rc::new(move || {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let st = state.borrow();
            let display = st.display_indices();
            let pinned = st.items.iter().filter(|i| i.pinned).count();
            title.set_subtitle(&format!("{} clips · {} pinned", st.items.len(), pinned));

            for &idx in display.iter() {
                let item = &st.items[idx];
                let row = gtk4::ListBoxRow::new();
                row.set_activatable(true);

                let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                hbox.set_margin_top(8);
                hbox.set_margin_bottom(8);
                hbox.set_margin_start(10);
                hbox.set_margin_end(10);

                let label = gtk4::Label::new(Some(&preview_text(&item.text)));
                label.set_hexpand(true);
                label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                label.set_xalign(0.0);
                hbox.append(&label);

                if item.pinned {
                    let pin = gtk4::Label::new(Some("Pinned"));
                    pin.add_css_class("accent");
                    pin.add_css_class("caption");
                    hbox.append(&pin);
                }
                let time = gtk4::Label::new(Some(&rel_time(item.ts)));
                time.add_css_class("dim-label");
                time.add_css_class("caption");
                hbox.append(&time);

                row.set_child(Some(&hbox));

                // Right-click opens the row menu
                let gesture = gtk4::GestureClick::new();
                gesture.set_button(3);
                let row_clone = row.clone();
                let popup_vi = popup_vi.clone();
                let popover = popover.clone();
                gesture.connect_pressed(move |_g, _n, _x, _y| {
                    row_clone.grab_focus();
                    let vi = row_clone.index() as usize;
                    *popup_vi.borrow_mut() = Some(vi);
                    let alloc = row_clone.allocation();
                    let rect = gtk4::gdk::Rectangle::new(0, 0, alloc.width(), alloc.height());
                    if popover.parent().is_some() {
                        popover.unparent();
                    }
                    popover.set_parent(&row_clone);
                    popover.set_pointing_to(Some(&rect));
                    popover.popup();
                });
                row.add_controller(gesture);

                list.append(&row);
            }
        })
    };
    rebuild();

    // Actions
    let act_settings = gio::SimpleAction::new("settings", None);
    {
        let state = state.clone();
        let window = window.clone();
        act_settings.connect_activate(move |_, _| show_preferences(&window, &state));
    }
    window.add_action(&act_settings);

    let act_clear = gio::SimpleAction::new("clear", None);
    {
        let state = state.clone();
        let rebuild = rebuild.clone();
        act_clear.connect_activate(move |_, _| {
            let mut st = state.borrow_mut();
            st.items.retain(|i| i.pinned);
            st.save();
            drop(st);
            rebuild();
        });
    }
    window.add_action(&act_clear);

    let act_quit = gio::SimpleAction::new("quit", None);
    {
        let state = state.clone();
        let app = app.clone();
        act_quit.connect_activate(move |_, _| {
            state.borrow().save();
            app.quit();
        });
    }
    app.add_action(&act_quit);
    app.set_accels_for_action("app.quit", &["<Control>Q"]);

    let act_row_copy = gio::SimpleAction::new("row-copy", None);
    {
        let state = state.clone();
        let window = window.clone();
        let rebuild = rebuild.clone();
        let popup_vi = popup_vi.clone();
        act_row_copy.connect_activate(move |_, _| {
            let Some(vi) = *popup_vi.borrow() else { return };
            let hide = copy_display(&state, vi);
            rebuild();
            if hide {
                window.set_visible(false);
            }
        });
    }
    window.add_action(&act_row_copy);

    let act_row_pin = gio::SimpleAction::new("row-pin", None);
    {
        let state = state.clone();
        let rebuild = rebuild.clone();
        let popup_vi = popup_vi.clone();
        act_row_pin.connect_activate(move |_, _| {
            let Some(vi) = *popup_vi.borrow() else { return };
            let mut st = state.borrow_mut();
            if let Some(idx) = st.display_indices().get(vi).copied() {
                if let Some(item) = st.items.get_mut(idx) {
                    item.pinned = !item.pinned;
                }
                st.save();
            }
            drop(st);
            rebuild();
        });
    }
    window.add_action(&act_row_pin);

    let act_row_delete = gio::SimpleAction::new("row-delete", None);
    {
        let state = state.clone();
        let rebuild = rebuild.clone();
        let popup_vi = popup_vi.clone();
        act_row_delete.connect_activate(move |_, _| {
            let Some(vi) = *popup_vi.borrow() else { return };
            let mut st = state.borrow_mut();
            if let Some(idx) = st.display_indices().get(vi).copied() {
                if idx < st.items.len() {
                    st.items.remove(idx);
                }
                st.save();
            }
            drop(st);
            rebuild();
        });
    }
    window.add_action(&act_row_delete);

    // Click or Enter on a row copies it
    {
        let state = state.clone();
        let window = window.clone();
        let rebuild = rebuild.clone();
        list.connect_row_activated(move |_l, row| {
            let vi = row.index() as usize;
            let hide = copy_display(&state, vi);
            rebuild();
            if hide {
                window.set_visible(false);
            }
        });
    }

    // Filter the list as the user types
    {
        let state_changed = state.clone();
        let rebuild_changed = rebuild.clone();
        search.connect_changed(move |e| {
            let q = e.text().to_string();
            state_changed.borrow_mut().query = q;
            rebuild_changed();
        });
        // Enter in the search box copies the first match
        let state_act = state.clone();
        let window_act = window.clone();
        let rebuild_act = rebuild.clone();
        search.connect_activate(move |_e| {
            let hide = copy_first(&state_act);
            rebuild_act();
            if hide {
                window_act.set_visible(false);
            }
        });
        // Down arrow moves from search into the list
        let list2 = list.clone();
        search.connect_next_match(move |_| {
            list2.grab_focus();
        });
    }

    // Escape clears the search, or hides the window if it is empty
    {
        let window_esc = window.clone();
        let state_esc = state.clone();
        let search_esc = search.clone();
        let key = gtk4::EventControllerKey::new();
        let key_ctrl = key.clone();
        key_ctrl.connect_key_pressed(move |_k, key: gtk4::gdk::Key, _kc, _mods| {
            if key == gtk4::gdk::Key::Escape {
                if state_esc.borrow().query.is_empty() {
                    window_esc.set_visible(false);
                } else {
                    search_esc.set_text("");
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(key);
    }

    // Closing hides the window (capture keeps running) unless set to quit
    {
        let state = state.clone();
        window.connect_close_request(move |win| {
            state.borrow().save();
            if state.borrow().settings.close_action == CloseAction::Quit {
                glib::Propagation::Proceed
            } else {
                win.set_visible(false);
                glib::Propagation::Stop
            }
        });
    }

    // Capture through GDK (used on GNOME). Wayland only reports clipboard
    // changes to the focused window, so this records copies made while Pastel
    // is focused, plus the latest clip each time the window gains focus.
    {
        let clipboard = gtk4::prelude::WidgetExt::display(&window).clipboard();
        let capture: Rc<dyn Fn(&gtk4::gdk::Clipboard)> = {
            let state = state.clone();
            let rebuild = rebuild.clone();
            Rc::new(move |cb: &gtk4::gdk::Clipboard| {
                let formats = cb.formats();
                if formats
                    .mime_types()
                    .iter()
                    .any(|m| m.contains("passwordManagerHint"))
                {
                    return;
                }
                let state = state.clone();
                let rebuild = rebuild.clone();
                cb.read_text_async(None::<&gio::Cancellable>, move |res| {
                    let Ok(Some(text)) = res else { return };
                    if text.as_str() == state.borrow().last_clip {
                        return;
                    }
                    state.borrow_mut().ingest(text.to_string());
                    state.borrow().save();
                    rebuild();
                });
            })
        };
        {
            let capture = capture.clone();
            clipboard.connect_changed(move |cb| capture(cb));
        }
        window.connect_is_active_notify(move |w| {
            if w.is_active() {
                capture(&gtk4::prelude::WidgetExt::display(w).clipboard());
            }
        });
    }

    // Capture through `wl-paste --watch` (wlroots compositors such as Sway or
    // Hyprland). It runs on its own thread; the main loop drains the channel.
    // GNOME lacks the protocol, so there the watcher exits right away.
    {
        let rx = spawn_wayland_watcher();
        let state = state.clone();
        let rebuild = rebuild.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            let mut changed = false;
            let mut alive = true;
            loop {
                match rx.try_recv() {
                    Ok(WatchEvent::Clip(text)) => {
                        state.borrow_mut().ingest(text);
                        changed = true;
                    }
                    Ok(WatchEvent::Dead) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        eprintln!("clipboard watcher stopped: `wl-paste --watch` unavailable");
                        alive = false;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
            if changed {
                state.borrow().save();
                rebuild();
            }
            if alive {
                ControlFlow::Continue
            } else {
                ControlFlow::Break
            }
        });
    }

    // Handle `pastel show` / `pastel quit` from other processes
    {
        let listener = listener.clone();
        let window = window.clone();
        let app = app.clone();
        let state_sock = state.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
            if let Some(listener) = listener.as_ref() {
                for _ in 0..4 {
                    let Ok((mut stream, _)) = listener.accept() else {
                        break;
                    };
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(80)));
                    let mut msg = String::new();
                    let mut buf = [0u8; 32];
                    while let Ok(n) = stream.read(&mut buf) {
                        if n == 0 || msg.len() > 16 {
                            break;
                        }
                        msg.push_str(&String::from_utf8_lossy(&buf[..n]));
                    }
                    if msg.starts_with("quit") {
                        state_sock.borrow().save();
                        app.quit();
                    } else {
                        window.present();
                    }
                }
            }
            ControlFlow::Continue
        });
    }

    // Save and remove the socket on exit
    {
        let state = state.clone();
        app.connect_shutdown(move |_| {
            state.borrow().save();
            let _ = std::fs::remove_file(socket_path());
        });
    }

    window.present();
}

// Preferences window

fn show_preferences(window: &adw::ApplicationWindow, state: &Rc<RefCell<AppState>>) {
    let prefs = adw::PreferencesWindow::new();
    prefs.set_title(Some("Preferences"));
    prefs.set_transient_for(Some(window));
    prefs.set_modal(true);

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    group.set_title("General");

    // History size
    let size_row = adw::ActionRow::builder()
        .title("History size")
        .subtitle("Unpinned clips kept")
        .build();
    let spin = gtk4::SpinButton::with_range(10.0, 2000.0, 1.0);
    spin.set_valign(gtk4::Align::Center);
    spin.set_value(state.borrow().settings.max_items as f64);
    {
        let state = state.clone();
        spin.connect_value_changed(move |s| {
            state.borrow_mut().settings.max_items = s.value() as usize;
            state.borrow().save();
        });
    }
    size_row.add_suffix(&spin);
    group.add(&size_row);

    // Hide after copy
    let hide_row = adw::ActionRow::builder()
        .title("Hide after copying")
        .subtitle("Paste immediately with Ctrl+V")
        .build();
    let hide_sw = gtk4::Switch::new();
    hide_sw.set_valign(gtk4::Align::Center);
    hide_sw.set_active(state.borrow().settings.hide_after_copy);
    {
        let state = state.clone();
        hide_sw.connect_active_notify(move |s| {
            state.borrow_mut().settings.hide_after_copy = s.is_active();
            state.borrow().save();
        });
    }
    hide_row.add_suffix(&hide_sw);
    group.add(&hide_row);

    // Quit on close
    let quit_row = adw::ActionRow::builder()
        .title("Quit when window is closed")
        .subtitle("Otherwise it keeps recording in the background")
        .build();
    let quit_sw = gtk4::Switch::new();
    quit_sw.set_valign(gtk4::Align::Center);
    quit_sw.set_active(state.borrow().settings.close_action == CloseAction::Quit);
    {
        let state = state.clone();
        quit_sw.connect_active_notify(move |s| {
            let mut st = state.borrow_mut();
            st.settings.close_action = if s.is_active() {
                CloseAction::Quit
            } else {
                CloseAction::Hide
            };
            st.save();
        });
    }
    quit_row.add_suffix(&quit_sw);
    group.add(&quit_row);

    // Start at login
    let auto_row = adw::ActionRow::builder()
        .title("Start at login")
        .subtitle("Record clipboard from boot")
        .build();
    let auto_sw = gtk4::Switch::new();
    auto_sw.set_valign(gtk4::Align::Center);
    auto_sw.set_active(state.borrow().settings.autostart);
    {
        let state = state.clone();
        auto_sw.connect_active_notify(move |s| {
            {
                let mut st = state.borrow_mut();
                st.settings.autostart = s.is_active();
                st.save();
            }
            state.borrow().apply_autostart();
        });
    }
    auto_row.add_suffix(&auto_sw);
    group.add(&auto_row);

    page.add(&group);

    // Maintenance
    let mgroup = adw::PreferencesGroup::new();
    mgroup.set_title("Maintenance");
    let clear_row = adw::ActionRow::builder()
        .title("Clear history")
        .subtitle("Removes all unpinned clips")
        .build();
    let clear_btn = gtk4::Button::with_label("Clear…");
    clear_btn.set_valign(gtk4::Align::Center);
    clear_btn.add_css_class("destructive-action");
    {
        let prefs2 = prefs.clone();
        let state = state.clone();
        clear_btn.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            st.items.retain(|i| i.pinned);
            st.save();
            drop(st);
            prefs2.close();
        });
    }
    clear_row.add_suffix(&clear_btn);
    mgroup.add(&clear_row);
    page.add(&mgroup);

    // About
    let agroup = adw::PreferencesGroup::new();
    let info_row = adw::ActionRow::builder()
        .title("History is stored locally")
        .subtitle(history_path().display().to_string())
        .build();
    agroup.add(&info_row);
    let tip_row = adw::ActionRow::builder()
        .title("Tip")
        .subtitle(format!("Run “{} install” to bind Super+V", exe_path()))
        .build();
    agroup.add(&tip_row);
    page.add(&agroup);

    prefs.add(&page);
    prefs.present();
}
