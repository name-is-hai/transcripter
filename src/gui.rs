use crate::MediaInfo;
use gpui::{
    App, Application, Bounds, ClickEvent, Context, Div, ElementId, PathPromptOptions, Render,
    SharedString, Stateful, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatusKind {
    Ready,
    Working,
    Success,
    Error,
}

struct TranscripterApp {
    videos: Vec<MediaInfo>,
    selected_video: Option<usize>,
    extract_language: String,
    output_dir: Option<PathBuf>,
    external_subtitle: Option<PathBuf>,
    status_kind: StatusKind,
    status: String,
    busy: bool,
}

impl TranscripterApp {
    fn new() -> Self {
        Self {
            videos: Vec::new(),
            selected_video: None,
            extract_language: "English".to_owned(),
            output_dir: None,
            external_subtitle: None,
            status_kind: StatusKind::Ready,
            status: "Choose one or more MKV files to begin.".to_owned(),
            busy: false,
        }
    }

    fn choose_videos(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Choose MKV videos".into()),
        });

        cx.spawn(async move |view, cx| {
            let Some(paths) = picker.await.ok().and_then(Result::ok).flatten() else {
                return;
            };
            if paths.is_empty() {
                return;
            }
            let count = paths.len();
            let _ = view.update(cx, |this, cx| {
                this.start_work(format!("Inspecting {count} video(s)…"), cx);
            });
            let result = cx
                .background_spawn(async move { crate::inspect_videos(&paths) })
                .await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(videos) => {
                    let count = videos.len();
                    this.videos = videos;
                    this.selected_video = (count > 0).then_some(0);
                    this.extract_language = "English".to_owned();
                    this.finish_success(format!("Loaded {count} video(s)."), cx);
                }
                Err(error) => this.finish_error(format!("{error:#}"), cx),
            });
        })
        .detach();
    }

    fn choose_output_directory(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose subtitle output folder".into()),
        });

        cx.spawn(async move |view, cx| {
            let Some(paths) = picker.await.ok().and_then(Result::ok).flatten() else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = view.update(cx, |this, cx| {
                this.output_dir = Some(path.clone());
                this.status_kind = StatusKind::Ready;
                this.status = format!("SRT files will be written to {}", path.display());
                cx.notify();
            });
        })
        .detach();
    }

    fn choose_external_subtitle(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose an SRT subtitle".into()),
        });

        cx.spawn(async move |view, cx| {
            let Some(paths) = picker.await.ok().and_then(Result::ok).flatten() else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = view.update(cx, |this, cx| {
                this.external_subtitle = Some(path.clone());
                this.status_kind = StatusKind::Ready;
                this.status = format!("Selected subtitle {}", path.display());
                cx.notify();
            });
        })
        .detach();
    }

    fn extract(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.videos.is_empty() {
            return;
        }
        let videos = self
            .videos
            .iter()
            .map(|video| video.path.clone())
            .collect::<Vec<_>>();
        let language = self.extract_language.clone();
        let output_dir = self.output_dir.clone();
        self.start_work(
            format!(
                "Extracting {language} subtitles from {} video(s)…",
                videos.len()
            ),
            cx,
        );

        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::extract_subtitles(&videos, &language, output_dir.as_deref())
                })
                .await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(outputs) => {
                    let paths = outputs
                        .iter()
                        .map(|output| output.subtitle.display().to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                    this.finish_success(format!("Created:\n{paths}"), cx);
                }
                Err(error) => this.finish_error(format!("{error:#}"), cx),
            });
        })
        .detach();
    }

    fn add_external(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(video) = self
            .selected_video
            .and_then(|index| self.videos.get(index))
            .map(|video| video.path.clone())
        else {
            self.finish_error("Select a video first.".to_owned(), cx);
            return;
        };
        let Some(subtitle) = self.external_subtitle.clone() else {
            self.finish_error("Choose an SRT subtitle first.".to_owned(), cx);
            return;
        };
        self.start_work(
            format!("Adding {} to {}…", subtitle.display(), video.display()),
            cx,
        );

        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::add_subtitle(&video, &subtitle, None, "eng", "English")
                })
                .await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(output) => {
                    this.finish_success(format!("Created {}", output.display()), cx);
                }
                Err(error) => this.finish_error(format!("{error:#}"), cx),
            });
        })
        .detach();
    }

    fn start_work(&mut self, message: String, cx: &mut Context<Self>) {
        self.busy = true;
        self.status_kind = StatusKind::Working;
        self.status = message;
        cx.notify();
    }

    fn finish_success(&mut self, message: String, cx: &mut Context<Self>) {
        self.busy = false;
        self.status_kind = StatusKind::Success;
        self.status = message;
        cx.notify();
    }

    fn finish_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.busy = false;
        self.status_kind = StatusKind::Error;
        self.status = message;
        cx.notify();
    }
}

impl Render for TranscripterApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_video = self.selected_video;
        let video_rows = self
            .videos
            .iter()
            .map(|video| {
                let track_names = video
                    .subtitle_tracks
                    .iter()
                    .map(|track| track.display_title())
                    .collect::<Vec<_>>()
                    .join(", ");
                (video.path.clone(), video.subtitle_tracks.len(), track_names)
            })
            .collect::<Vec<_>>();
        let track_rows = selected_video
            .and_then(|index| self.videos.get(index))
            .map(|video| {
                video
                    .subtitle_tracks
                    .iter()
                    .map(|track| (track.stream_index, track.display_title().to_owned()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let output_label = self
            .output_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Output: next to each video".to_owned());
        let subtitle_label = self
            .external_subtitle
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Choose an external SRT".to_owned());
        let status_color = match self.status_kind {
            StatusKind::Ready => rgb(0xcbd5e1),
            StatusKind::Working => rgb(0x60a5fa),
            StatusKind::Success => rgb(0x4ade80),
            StatusKind::Error => rgb(0xf87171),
        };

        let videos = if video_rows.is_empty() {
            div()
                .p_3()
                .text_color(rgb(0x94a3b8))
                .child("No videos selected")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(video_rows.into_iter().enumerate().map(
                    |(index, (path, track_count, track_names))| {
                        let details = if track_count == 0 {
                            "No embedded subtitle tracks".to_owned()
                        } else {
                            format!("{track_count} tracks: {track_names}")
                        };
                        control(
                            ("video", index),
                            format!("{}\n{details}", path.display()),
                            false,
                            selected_video == Some(index),
                            self.busy,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.busy {
                                this.selected_video = Some(index);
                                cx.notify();
                            }
                        }))
                    },
                ))
                .into_any_element()
        };

        let tracks = if track_rows.is_empty() {
            div()
                .text_color(rgb(0x94a3b8))
                .child("Select a video to see its tracks")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(track_rows.into_iter().map(|(stream_index, title)| {
                    let selected = self.extract_language.eq_ignore_ascii_case(&title);
                    let chosen_title = title.clone();
                    control(("track", stream_index), title, false, selected, self.busy).on_click(
                        cx.listener(move |this, _, _, cx| {
                            if !this.busy {
                                this.extract_language = chosen_title.clone();
                                cx.notify();
                            }
                        }),
                    )
                }))
                .into_any_element()
        };

        div()
            .id("app-scroll")
            .size_full()
            .overflow_scroll()
            .bg(rgb(0x0f172a))
            .text_color(rgb(0xf8fafc))
            .child(
                div()
                    .w_full()
                    .max_w(px(1000.))
                    .mx_auto()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_2xl().child("Transcripter"))
                            .child(div().text_color(rgb(0x94a3b8)).child(
                                "Extract or add subtitle tracks without re-encoding video.",
                            )),
                    )
                    .child(
                        card()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .gap_3()
                                    .child(div().text_lg().child("1. Videos"))
                                    .child(
                                        control(
                                            "choose-videos",
                                            "Choose MKV videos",
                                            true,
                                            false,
                                            self.busy,
                                        )
                                        .on_click(cx.listener(Self::choose_videos)),
                                    ),
                            )
                            .child(videos),
                    )
                    .child(
                        card()
                            .child(div().text_lg().child("2. Extract embedded subtitles"))
                            .child(
                                div()
                                    .text_color(rgb(0x94a3b8))
                                    .child(format!("Selected track: {}", self.extract_language)),
                            )
                            .child(tracks)
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        control(
                                            "choose-output",
                                            output_label,
                                            false,
                                            false,
                                            self.busy,
                                        )
                                        .on_click(cx.listener(Self::choose_output_directory)),
                                    )
                                    .child(
                                        control(
                                            "extract",
                                            "Extract SRT",
                                            true,
                                            false,
                                            self.busy || self.videos.is_empty(),
                                        )
                                        .on_click(cx.listener(Self::extract)),
                                    ),
                            ),
                    )
                    .child(
                        card()
                            .child(div().text_lg().child("3. Add an external subtitle"))
                            .child(div().text_color(rgb(0x94a3b8)).child(
                                "The selected SRT is added as English to the selected video.",
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        control(
                                            "choose-srt",
                                            subtitle_label,
                                            false,
                                            false,
                                            self.busy,
                                        )
                                        .on_click(cx.listener(Self::choose_external_subtitle)),
                                    )
                                    .child(
                                        control(
                                            "add-subtitle",
                                            "Create subtitled MKV",
                                            true,
                                            false,
                                            self.busy
                                                || self.selected_video.is_none()
                                                || self.external_subtitle.is_none(),
                                        )
                                        .on_click(cx.listener(Self::add_external)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(0x1e293b))
                            .text_color(status_color)
                            .child(self.status.clone()),
                    )
                    .child(div().text_sm().text_color(rgb(0x64748b)).child(
                        "Release builds include FFmpeg under LGPL-2.1-or-later. Source: ffmpeg.org",
                    )),
            )
    }
}

fn card() -> Div {
    div()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x334155))
        .bg(rgb(0x111827))
}

fn control(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    selected: bool,
    disabled: bool,
) -> Stateful<Div> {
    let background = if primary {
        rgb(0x2563eb)
    } else if selected {
        rgb(0x334155)
    } else {
        rgb(0x1e293b)
    };
    let border = if selected {
        rgb(0x60a5fa)
    } else {
        rgb(0x475569)
    };

    div()
        .id(id)
        .min_h(px(40.))
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(border)
        .bg(background)
        .text_color(rgb(0xf8fafc))
        .when(!disabled, |this| {
            this.cursor_pointer().hover(|this| this.bg(rgb(0x3b82f6)))
        })
        .when(disabled, |this| this.opacity(0.45))
        .child(label.into())
}

pub fn run() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(680.), px(560.))),
                app_id: Some("dev.transcripter.app".to_owned()),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Transcripter");
                cx.new(|_| TranscripterApp::new())
            },
        )
        .expect("failed to open Transcripter window");
        cx.activate(true);
    });
}
