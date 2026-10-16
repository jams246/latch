use super::{Files, pane::Side};
use crate::ui::button;
use gpui::{prelude::*, *};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod drive_tests;
#[cfg(test)]
mod scroll_tests;

impl Files {
    pub(super) fn reveal_breadcrumbs(&mut self, side: Side) {
        self.breadcrumb_reveal[usize::from(side == Side::Remote)] = true;
    }

    pub(in crate::ui) fn scroll_breadcrumbs(&self, side: Side) {
        let target = match side {
            Side::Local => self.local.as_ref().map(|directory| {
                (
                    &self.local_breadcrumb_scroll,
                    local_crumbs(&directory.path).len(),
                )
            }),
            Side::Remote => self.directory.as_ref().map(|directory| {
                (
                    &self.remote_breadcrumb_scroll,
                    remote_crumbs(&directory.path).len(),
                )
            }),
        };
        if let Some((scroll, count)) = target {
            scroll.scroll_to_item(count.saturating_sub(1));
        }
    }

    pub(in crate::ui) fn breadcrumbs(&self, side: Side, cx: &mut Context<Self>) -> Div {
        let crumbs: Vec<AnyElement> = match side {
            Side::Local => self.local.as_ref().map_or_else(Vec::new, |directory| {
                local_crumbs(&directory.path)
                    .into_iter()
                    .enumerate()
                    .map(|(index, (label, path))| {
                        button(("crumb", index), label)
                            .debug_selector(move || format!("{}-crumb-{index}", side.id()))
                            .px_2()
                            .flex_shrink_0()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigate_local(path.clone(), cx);
                            }))
                            .into_any_element()
                    })
                    .collect()
            }),
            Side::Remote => self.directory.as_ref().map_or_else(Vec::new, |directory| {
                remote_crumbs(&directory.path)
                    .into_iter()
                    .enumerate()
                    .map(|(index, (label, path))| {
                        button(("crumb", index), label)
                            .debug_selector(move || format!("{}-crumb-{index}", side.id()))
                            .px_2()
                            .flex_shrink_0()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigate(path.clone(), cx);
                            }))
                            .into_any_element()
                    })
                    .collect()
            }),
        };
        div().flex().items_center().h_8().gap_1().child(
            div()
                .id("breadcrumbs")
                .debug_selector(move || format!("{}-breadcrumbs", side.id()))
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_1()
                .overflow_x_scroll()
                .track_scroll(match side {
                    Side::Local => &self.local_breadcrumb_scroll,
                    Side::Remote => &self.remote_breadcrumb_scroll,
                })
                .children(crumbs.into_iter().enumerate().map(|(index, crumb)| {
                    div()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .when(index > 0, |row| {
                            row.child(div().text_color(rgb(0x7893a8)).child("/"))
                        })
                        .child(crumb)
                })),
        )
    }

    pub(in crate::ui) fn address_row(&self, side: Side, cx: &mut Context<Self>) -> AnyElement {
        let row = div().flex().h_8().items_center().gap_2();
        match side {
            Side::Local => row
                .when_some(
                    self.local
                        .as_ref()
                        .filter(|directory| directory.drives.len() > 1),
                    |row, directory| {
                        row.child(
                            div()
                                .debug_selector(|| "local-drives-label".into())
                                .text_xs()
                                .text_color(rgb(0xa8c1d3))
                                .child("Drives"),
                        )
                        .child(
                            div()
                                .id("drives")
                                .debug_selector(|| "local-drives".into())
                                .flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .overflow_x_scroll()
                                .children(directory.drives.iter().enumerate().map(
                                    |(index, drive)| {
                                        let path = drive.clone();
                                        button(("drive", index), display_path(drive))
                                            .debug_selector(move || format!("local-drive-{index}"))
                                            .px_2()
                                            .flex_shrink_0()
                                            .when(directory.path.starts_with(drive), |button| {
                                                button.bg(rgb(0x426c80))
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.navigate_local(path.clone(), cx);
                                            }))
                                    },
                                )),
                        )
                    },
                )
                .into_any_element(),
            Side::Remote => row
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(self.path.clone())
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "enter" {
                                this.navigate(this.path.read(cx).value.to_string(), cx);
                            }
                        })),
                )
                .child(button("go", "Go").on_click(cx.listener(|this, _, _, cx| {
                    this.navigate(this.path.read(cx).value.to_string(), cx);
                })))
                .into_any_element(),
        }
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string()
}

fn local_crumbs(path: &Path) -> Vec<(String, PathBuf)> {
    let mut crumbs: Vec<_> = path
        .ancestors()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| {
            let label = path.file_name().map_or_else(
                || display_path(path),
                |name| name.to_string_lossy().into_owned(),
            );
            (label, path.to_path_buf())
        })
        .collect();
    crumbs.reverse();
    crumbs
}

fn remote_crumbs(path: &str) -> Vec<(String, String)> {
    let mut crumbs = vec![("/".into(), "/".into())];
    let mut current = String::new();
    for part in path.split('/').filter(|part| !part.is_empty()) {
        current.push('/');
        current.push_str(part);
        crumbs.push((part.to_string(), current.clone()));
    }
    crumbs
}

#[cfg(test)]
mod tests {
    use super::remote_crumbs;
    #[cfg(windows)]
    use {
        super::local_crumbs,
        std::path::{Path, PathBuf},
    };

    #[test]
    fn remote_breadcrumbs_keep_clickable_absolute_paths() {
        assert_eq!(
            remote_crumbs("/home/test"),
            vec![
                ("/".into(), "/".into()),
                ("home".into(), "/home".into()),
                ("test".into(), "/home/test".into())
            ]
        );
    }

    #[test]
    fn remote_root_has_one_breadcrumb() {
        assert_eq!(remote_crumbs("/"), vec![("/".into(), "/".into())]);
    }

    #[cfg(windows)]
    #[test]
    fn local_breadcrumbs_include_drive_root_and_keep_native_paths() {
        assert_eq!(
            local_crumbs(Path::new(r"C:\Users\test")),
            vec![
                (r"C:\".into(), PathBuf::from(r"C:\")),
                ("Users".into(), PathBuf::from(r"C:\Users")),
                ("test".into(), PathBuf::from(r"C:\Users\test"))
            ]
        );
    }
}
