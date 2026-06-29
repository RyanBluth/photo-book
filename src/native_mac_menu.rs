use std::{path::PathBuf, sync::Mutex};

use objc2::{
    ClassType, MainThreadMarker, define_class, msg_send, rc::Retained, runtime::AnyObject, sel,
};
use objc2_app_kit::{NSApp, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{NSObject, NSString};
use once_cell::sync::Lazy;

use crate::{
    auto_persisting::AutoPersisting, config::Config, scene::organize_edit_scene::MenuCommand,
};

static PENDING_COMMANDS: Lazy<Mutex<Vec<MenuCommand>>> = Lazy::new(|| Mutex::new(Vec::new()));
static RECENT_PROJECTS: Lazy<Mutex<Vec<PathBuf>>> = Lazy::new(|| Mutex::new(Vec::new()));

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "PhotoBookMenuHandler"]
    pub struct NativeMenuHandler;

    impl NativeMenuHandler {
        #[unsafe(method(init))]
        fn init(&self) -> *mut Self {
            unsafe { msg_send![super(self), init] }
        }

        #[unsafe(method(newCollection:))]
        fn new_collection(&self, _sender: &AnyObject) {
            push_command(MenuCommand::NewCollection);
        }

        #[unsafe(method(openCollection:))]
        fn open_collection(&self, _sender: &AnyObject) {
            push_command(MenuCommand::OpenCollection);
        }

        #[unsafe(method(openRecentCollection:))]
        fn open_recent_collection(&self, sender: &AnyObject) {
            let Some(item) = sender.downcast_ref::<NSMenuItem>() else {
                return;
            };

            let tag = item.tag();
            if tag < 0 {
                return;
            }

            let Some(path) = RECENT_PROJECTS.lock().unwrap().get(tag as usize).cloned() else {
                return;
            };

            push_command(MenuCommand::OpenRecent(path));
        }

        #[unsafe(method(saveCollection:))]
        fn save_collection(&self, _sender: &AnyObject) {
            push_command(MenuCommand::Save);
        }

        #[unsafe(method(importPhotos:))]
        fn import_photos(&self, _sender: &AnyObject) {
            push_command(MenuCommand::Import);
        }

        #[unsafe(method(exportBook:))]
        fn export_book(&self, _sender: &AnyObject) {
            push_command(MenuCommand::Export);
        }

        #[unsafe(method(groupByDate:))]
        fn group_by_date(&self, _sender: &AnyObject) {
            push_command(MenuCommand::GroupByDate);
        }

        #[unsafe(method(groupByRating:))]
        fn group_by_rating(&self, _sender: &AnyObject) {
            push_command(MenuCommand::GroupByRating);
        }

        #[unsafe(method(openPageSettings:))]
        fn open_page_settings(&self, _sender: &AnyObject) {
            push_command(MenuCommand::PageSettings);
        }

        #[unsafe(method(toggleQuickLayoutNumbers:))]
        fn toggle_quick_layout_numbers(&self, _sender: &AnyObject) {
            push_command(MenuCommand::ToggleQuickLayoutNumbers);
        }

        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, _item: &NSMenuItem) -> bool {
            true
        }
    }
);

pub fn install() -> Option<Retained<NativeMenuHandler>> {
    let handler = make_handler()?;
    create_global_menu(&handler);
    Some(handler)
}

pub fn drain_commands() -> Vec<MenuCommand> {
    PENDING_COMMANDS.lock().unwrap().drain(..).collect()
}

fn push_command(command: MenuCommand) {
    PENDING_COMMANDS.lock().unwrap().push(command);
}

fn make_handler() -> Option<Retained<NativeMenuHandler>> {
    unsafe {
        let obj: *mut NativeMenuHandler = msg_send![NativeMenuHandler::class(), alloc];
        Retained::from_raw(msg_send![obj, init])
    }
}

fn create_global_menu(handler: &NativeMenuHandler) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    let app = NSApp(mtm);
    let Some(main_menu) = app.mainMenu() else {
        return;
    };

    while main_menu.numberOfItems() > 1 {
        main_menu.removeItemAtIndex(main_menu.numberOfItems() - 1);
    }

    let recents = crate::dep_mut!(AutoPersisting<Config>, |config| {
        config.read().unwrap().recent_projects().to_vec()
    });
    *RECENT_PROJECTS.lock().unwrap() = recents.clone();

    add_submenu(mtm, &main_menu, "File", |menu| {
        add_command_item(
            mtm,
            menu,
            "New Collection",
            sel!(newCollection:),
            "n",
            NSEventModifierFlags::Command,
            handler,
        );
        add_command_item(
            mtm,
            menu,
            "Open Collection...",
            sel!(openCollection:),
            "o",
            NSEventModifierFlags::Command,
            handler,
        );
        add_recent_submenu(mtm, menu, &recents, handler);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        add_command_item(
            mtm,
            menu,
            "Save",
            sel!(saveCollection:),
            "s",
            NSEventModifierFlags::Command,
            handler,
        );
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        add_command_item(
            mtm,
            menu,
            "Import...",
            sel!(importPhotos:),
            "",
            NSEventModifierFlags::empty(),
            handler,
        );
        add_command_item(
            mtm,
            menu,
            "Export...",
            sel!(exportBook:),
            "e",
            NSEventModifierFlags::Command,
            handler,
        );
    });

    add_submenu(mtm, &main_menu, "Group By", |menu| {
        add_command_item(
            mtm,
            menu,
            "Date",
            sel!(groupByDate:),
            "",
            NSEventModifierFlags::empty(),
            handler,
        );
        add_command_item(
            mtm,
            menu,
            "Rating",
            sel!(groupByRating:),
            "",
            NSEventModifierFlags::empty(),
            handler,
        );
    });

    add_submenu(mtm, &main_menu, "Collection Settings", |menu| {
        add_command_item(
            mtm,
            menu,
            "Page Settings...",
            sel!(openPageSettings:),
            ",",
            NSEventModifierFlags::Command,
            handler,
        );
    });

    add_submenu(mtm, &main_menu, "Debug", |menu| {
        add_command_item(
            mtm,
            menu,
            "Quick Layout Numbers",
            sel!(toggleQuickLayoutNumbers:),
            "q",
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
            handler,
        );
    });
}

fn add_recent_submenu(
    mtm: MainThreadMarker,
    file_menu: &NSMenu,
    recents: &[PathBuf],
    handler: &NativeMenuHandler,
) {
    let menu_item = NSMenuItem::new(mtm);
    menu_item.setTitle(&NSString::from_str("Open Recent"));
    let recent_menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str("Open Recent"));

    if recents.is_empty() {
        let empty_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("No recent collections"),
                None,
                &NSString::from_str(""),
            )
        };
        empty_item.setEnabled(false);
        recent_menu.addItem(&empty_item);
    } else {
        for (index, path) in recents.iter().enumerate() {
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc(),
                    &NSString::from_str(&path.display().to_string()),
                    Some(sel!(openRecentCollection:)),
                    &NSString::from_str(""),
                )
            };
            item.setTag(index as isize);
            item.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
            unsafe {
                item.setTarget(Some(handler as &AnyObject));
            }
            recent_menu.addItem(&item);
        }
    }

    menu_item.setSubmenu(Some(&recent_menu));
    file_menu.addItem(&menu_item);
}

fn add_submenu(
    mtm: MainThreadMarker,
    main_menu: &NSMenu,
    title: &str,
    add_items: impl FnOnce(&NSMenu),
) {
    let menu_item = NSMenuItem::new(mtm);
    let menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(title));
    add_items(&menu);
    menu_item.setSubmenu(Some(&menu));
    main_menu.addItem(&menu_item);
}

fn add_command_item(
    mtm: MainThreadMarker,
    menu: &NSMenu,
    title: &str,
    selector: objc2::runtime::Sel,
    key: &str,
    modifiers: NSEventModifierFlags,
    handler: &NativeMenuHandler,
) {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            Some(selector),
            &NSString::from_str(key),
        )
    };
    item.setKeyEquivalentModifierMask(modifiers);
    unsafe {
        item.setTarget(Some(handler as &AnyObject));
    }
    menu.addItem(&item);
}
