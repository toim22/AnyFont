//! AnyFont — font manager for Windows. (Main window owned by the shell UI developer; this is the integration stub.)
mod about;
mod bugreport;
mod getmore;
mod help;
mod matching;
mod updater;

fn main() {
    let _ = (about::About::new(), bugreport::BugReport::new(), getmore::GetMore::new(), matching::Matching::new(), updater::Updater::new());
    help::open_product_help();
}
