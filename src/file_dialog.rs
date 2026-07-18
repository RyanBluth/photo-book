use std::{future::Future, path::PathBuf};

use egui::Context;

pub type FileDialogResult = native_dialog::Result<Option<PathBuf>>;

pub fn spawn<F, C>(dialog: F, ctx: Context, complete: C)
where
    F: Future<Output = FileDialogResult> + Send + 'static,
    C: FnOnce(FileDialogResult, &Context) + Send + 'static,
{
    tokio::spawn(async move {
        complete(dialog.await, &ctx);
        ctx.request_repaint();
    });
}
