//! Enable/disable rules of the toolbar, the main menu and the context menus.
//!
//! The original computes these as `FontDS` properties (`InstallEnabled`, `UninstallEnabled`,
//! `DeleteEnabled`, `DownloadEnabled`, `RemoveFolderOrCollectionEnabled` ...; dossier
//! `06-ui/handler-map.md`). The rules below reproduce all 13 captured toolbar states
//! (`06-ui/screenshots/90-reports/toolbar-states.json`, encoded as a unit test):
//!
//! * Add Folder, Add Collection, Get More, Show duplicate fonts, Show font properties: always enabled.
//! * Remove Folder or Collection: the current library is a user folder or a collection (Favorites included;
//!   removing Favorites empties it), never "My fonts".
//! * Install: a selected face is not installed, usable and local.
//! * Uninstall: a selected face is installed and not a protected system font.
//! * Delete: a selected face has a local file and is not protected.
//! * Download: a selected face is online-only.
//! * A selected family header is not a face selection for the toolbar (state `family-header-selected`).
//!   The Font menu / context menu counts it as "all faces of the family" (`docs/UI-SPEC.md` §2.6), so the
//!   `menu_*` fields include the header's faces.
//! * Add to collection: there is a selection or a current font. Remove from {open collection}: a collection
//!   is open. Remove from collection ▸ (outside collections): a selected face or the current font belongs to
//!   a collection. Open font file location: exactly one face or one family header selected.

use crate::catalog::LibraryRef;
use crate::model::FontRecord;
use serde::Serialize;

/// The current library and selection.
#[derive(Clone, Debug, Default)]
pub struct Selection<'a> {
    /// Library shown in the list.
    pub library: LibraryRef,
    /// True for the Favorites collection.
    pub library_is_favorites: bool,
    /// Faces selected explicitly (font rows).
    pub faces: Vec<&'a FontRecord>,
    /// Faces of the selected family header rows (context menus only).
    pub header_faces: Vec<&'a FontRecord>,
    /// "Select all" is active.
    pub all_selected: bool,
    /// The current font (last focused face; stays highlighted after Deselect All).
    pub current: Option<&'a FontRecord>,
}

/// Enabled state of every command that depends on the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Default)]
pub struct Eligibility {
    /// Toolbar "Add Folder".
    pub add_folder: bool,
    /// Toolbar "Add Collection".
    pub add_collection: bool,
    /// Toolbar "Get More".
    pub get_more: bool,
    /// Toolbar / library context menu "Remove Folder or Collection".
    pub remove_folder_or_collection: bool,
    /// Library context menu "Rename" (collections other than Favorites).
    pub rename_library: bool,
    /// Install.
    pub install: bool,
    /// Uninstall.
    pub uninstall: bool,
    /// Download.
    pub download: bool,
    /// Delete.
    pub delete: bool,
    /// Toolbar toggle "Show duplicate fonts".
    pub show_duplicates: bool,
    /// Toolbar toggle "Show font properties".
    pub show_font_properties: bool,
    /// Font menu / context menu Install (family headers count as their faces).
    pub menu_install: bool,
    /// Font menu / context menu Uninstall.
    pub menu_uninstall: bool,
    /// Font menu / context menu Download.
    pub menu_download: bool,
    /// Font menu / context menu Delete.
    pub menu_delete: bool,
    /// "Open font file location ▸".
    pub open_file_location: bool,
    /// "Add to collection ▸".
    pub add_to_collection: bool,
    /// "Remove from {collection}" (visible only inside a collection).
    pub remove_from_open_collection: bool,
    /// "Remove from collection ▸" (outside collections).
    pub remove_from_collection: bool,
    /// Edit > Deselect All.
    pub deselect_all: bool,
}

impl<'a> Selection<'a> {
    fn context_faces(&self) -> impl Iterator<Item = &&'a FontRecord> {
        self.faces.iter().chain(self.header_faces.iter())
    }
}

fn can_install(f: &FontRecord) -> bool {
    !f.installed && f.is_usable()
}
fn can_uninstall(f: &FontRecord) -> bool {
    f.installed && !f.protected
}
fn can_delete(f: &FontRecord) -> bool {
    !f.paths.is_empty() && !f.protected
}

/// Evaluates every rule.
pub fn evaluate(sel: &Selection) -> Eligibility {
    let user_library = !matches!(sel.library, LibraryRef::MyFonts);
    let in_collection = matches!(sel.library, LibraryRef::Collection(_));
    let ctx: Vec<&FontRecord> = sel.context_faces().copied().collect();
    let header_one_family = {
        let mut fams: Vec<&str> = sel.header_faces.iter().map(|f| f.group_family()).collect();
        fams.sort_unstable();
        fams.dedup();
        fams.len() == 1
    };
    let one_face_or_header = (sel.faces.len() == 1 && sel.header_faces.is_empty())
        || (sel.faces.is_empty() && header_one_family);
    let target: Vec<&FontRecord> = if ctx.is_empty() { sel.current.into_iter().collect() } else { ctx.clone() };
    Eligibility {
        add_folder: true,
        add_collection: true,
        get_more: true,
        remove_folder_or_collection: user_library,
        rename_library: in_collection && !sel.library_is_favorites,
        install: sel.faces.iter().any(|f| can_install(f)),
        uninstall: sel.faces.iter().any(|f| can_uninstall(f)),
        download: sel.faces.iter().any(|f| f.needs_download()),
        delete: sel.faces.iter().any(|f| can_delete(f)),
        show_duplicates: true,
        show_font_properties: true,
        menu_install: ctx.iter().any(|f| can_install(f)),
        menu_uninstall: ctx.iter().any(|f| can_uninstall(f)),
        menu_download: ctx.iter().any(|f| f.needs_download()),
        menu_delete: ctx.iter().any(|f| can_delete(f)),
        open_file_location: !sel.all_selected && one_face_or_header && ctx.iter().any(|f| !f.paths.is_empty()),
        add_to_collection: !target.is_empty(),
        remove_from_open_collection: in_collection,
        remove_from_collection: !in_collection && target.iter().any(|f| !f.collections.is_empty()),
        deselect_all: !sel.faces.is_empty() || !sel.header_faces.is_empty(),
    }
}

/// Context menu of a library entry (left panel): Install / Uninstall the whole library, Remove.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Default)]
pub struct LibraryMenu {
    /// Install every not-installed font of the library.
    pub install: bool,
    /// Uninstall every installed, unprotected font of the library.
    pub uninstall: bool,
    /// Remove Folder or Collection.
    pub remove: bool,
    /// Rename (collections other than Favorites).
    pub rename: bool,
}

/// Evaluates the library context menu for `fonts` (the library's faces).
pub fn library_menu(library: &LibraryRef, is_favorites: bool, fonts: &[&FontRecord]) -> LibraryMenu {
    LibraryMenu {
        install: fonts.iter().any(|f| can_install(f)),
        uninstall: fonts.iter().any(|f| can_uninstall(f)),
        remove: !matches!(library, LibraryRef::MyFonts),
        rename: matches!(library, LibraryRef::Collection(_)) && !is_favorites,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::BTreeMap;

    fn font(family: &str, installed: bool, protected: bool) -> FontRecord {
        FontRecord {
            family: family.into(),
            installed,
            protected,
            paths: vec![format!("C:/Windows/Fonts/{family}.ttf").into()],
            ..Default::default()
        }
    }

    /// Encodes `toolbar-states.json`: each captured state is rebuilt as a [`Selection`] and the toolbar
    /// buttons must match the capture exactly.
    #[test]
    fn toolbar_states_match_capture() {
        let text = include_str!("../tests/fixtures/toolbar-states.json");
        let json: serde_json::Value = serde_json::from_str(text).unwrap();
        let matrix = json["matrix"].as_object().unwrap();

        let algerian = font("Algerian", true, false); // installed user font
        let arial = font("Arial", true, true); // protected system font
        let temp_copy = font("Agency FB", true, false); // copy of an installed font in a temp folder
        let all = [&algerian, &arial, &temp_copy];

        let states: BTreeMap<&str, Selection> = [
            ("all-selected", Selection { faces: all.to_vec(), all_selected: true, ..Default::default() }),
            // deselect-all: no face selected (the current font Arial stays in the preview only)
            ("deselect-all", Selection::default()),
            // show duplicates rebuilt the list and cleared the selection
            ("duplicates-shown", Selection::default()),
            ("family-header-selected", Selection { header_faces: vec![&algerian], ..Default::default() }),
            // Favorites is empty: nothing to select
            (
                "favorites-library",
                Selection { library: LibraryRef::Collection(1), library_is_favorites: true, ..Default::default() },
            ),
            ("filter-no-results", Selection::default()),
            ("font-properties-shown", Selection { faces: vec![&algerian], ..Default::default() }),
            ("font-selected", Selection { faces: vec![&algerian], ..Default::default() }),
            ("system-font-selected", Selection { faces: vec![&arial], ..Default::default() }),
            (
                "temp-folder-font-selected",
                Selection { library: LibraryRef::Folder(1), faces: vec![&temp_copy], ..Default::default() },
            ),
            // opening a library selects its first font
            (
                "temp-folder-library",
                Selection { library: LibraryRef::Folder(1), faces: vec![&temp_copy], ..Default::default() },
            ),
            (
                "user-collection-font-selected",
                Selection { library: LibraryRef::Collection(2), faces: vec![&algerian], ..Default::default() },
            ),
            (
                "user-collection-library",
                Selection { library: LibraryRef::Collection(2), faces: vec![&algerian], ..Default::default() },
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(states.len(), matrix.len(), "every captured state is encoded");
        for (state, expected) in matrix {
            let sel = states.get(state.as_str()).unwrap_or_else(|| panic!("state {state} not encoded"));
            let e = evaluate(sel);
            let actual = [
                ("Add Folder", e.add_folder),
                ("Add Collection", e.add_collection),
                ("Get More", e.get_more),
                ("Remove Folder or Collection", e.remove_folder_or_collection),
                ("Install", e.install),
                ("Uninstall", e.uninstall),
                ("Download", e.download),
                ("Delete", e.delete),
                ("Show duplicate fonts", e.show_duplicates),
                ("Show font properties", e.show_font_properties),
            ];
            let exp = expected.as_object().unwrap();
            assert_eq!(exp.len(), actual.len(), "{state}: button set");
            for (button, value) in actual {
                assert_eq!(exp[button].as_bool(), Some(value), "{state}: {button}");
            }
        }
    }

    #[test]
    fn install_download_and_context_rules() {
        let mut loose = font("Loose", false, false);
        let sel = Selection { faces: vec![&loose], ..Default::default() };
        let e = evaluate(&sel);
        assert!(e.install && !e.uninstall && e.delete && !e.download);
        loose.corrupt = Some("bad".into());
        assert!(!evaluate(&Selection { faces: vec![&loose], ..Default::default() }).install);
        let remote = FontRecord { family: "Web".into(), remote: Some(RemoteInfo::default()), ..Default::default() };
        let e = evaluate(&Selection { faces: vec![&remote], ..Default::default() });
        assert!(e.download && !e.delete && !e.install);

        let a = font("Arial", true, true);
        let b = font("Bahnschrift", true, true);
        let header = Selection { header_faces: vec![&a], ..Default::default() };
        assert!(evaluate(&header).open_file_location, "family header counts for the context menu");
        let all = Selection { faces: vec![&a, &b], all_selected: true, ..Default::default() };
        assert!(!evaluate(&all).open_file_location);
        let in_coll = Selection { library: LibraryRef::Collection(3), faces: vec![&a], ..Default::default() };
        let e = evaluate(&in_coll);
        assert!(e.remove_from_open_collection && !e.remove_from_collection && e.rename_library);
        let mut member = font("Algerian", true, false);
        member.collections = vec![3];
        let current_only = Selection { library: LibraryRef::Folder(1), current: Some(&member), ..Default::default() };
        let e = evaluate(&current_only);
        assert!(e.add_to_collection && e.remove_from_collection && !e.deselect_all && !e.uninstall);
        let header = Selection { header_faces: vec![&member], ..Default::default() };
        let e = evaluate(&header);
        assert!(e.menu_uninstall && e.menu_delete && !e.uninstall && !e.delete, "C2: menu yes, toolbar no");
        let m = library_menu(&LibraryRef::MyFonts, false, &[&a]);
        assert!(!m.remove && !m.uninstall && !m.install);
    }
}
