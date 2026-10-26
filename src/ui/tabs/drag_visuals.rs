use crate::ui::Latch;
use gpui::*;

impl Latch {
    pub(super) fn update_drag_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.tab_drag.moving.as_ref() else {
            return;
        };
        if !self.tab_drag.did_drag || !self.movable_tab(id) || self.tab_drag.preview_failed {
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            if self.tab_drag.preview.is_none() {
                let Some(tab) = self.tabs.iter().find(|tab| tab.id == *id) else {
                    return Ok(());
                };
                self.tab_drag.preview = Some(super::preview::Preview::new(
                    super::native::hwnd(window)?,
                    &tab.name,
                )?);
            }
            if let Some(preview) = &mut self.tab_drag.preview {
                preview.update(self.tab_drag.target.is_some())?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.tab_drag.preview = None;
            self.tab_drag.preview_failed = true;
            self.error = Some(format!("Cannot show tab drag preview: {error}"));
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::sessions::tests::fixture;
    use gpui::TestAppContext;

    #[gpui::test]
    fn drag_indicator_waits_for_threshold_and_resets_on_cancel(cx: &mut TestAppContext) {
        let (handle, connection) = fixture(cx);
        handle
            .update(cx, |app, _, cx| {
                let (tab, _) = crate::ui::sessions::tests::synthetic_tab(
                    connection,
                    crate::ui::SessionKind::Terminal,
                    cx,
                );
                let id = tab.id.clone();
                app.tabs.push(tab);
                app.tab_drag.moving = Some(id.clone());
                assert!(!app.tab_drag.dragging(&id));
                app.tab_drag.did_drag = true;
                assert!(app.tab_drag.dragging(&id));
                assert!(!app.tab_drag.dragging("another-tab"));
                app.tab_drag.preview_failed = true;
                app.cancel_tab_drag(cx);
                assert!(!app.tab_drag.dragging(&id));
                assert!(!app.tab_drag.preview_failed);
                assert!(app.tab_drag.preview.is_none());
            })
            .unwrap();
    }
}
